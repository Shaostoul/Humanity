//! Putting a file that is on the server on the Files page (`POST /api/files`), and undoing it when
//! Cancel came while it was on its way (round 3 of docs/design/files-showcase.md, the 2026-10-11
//! review's findings 1 and 11).
//!
//! EACH PUBLISH BELONGS TO WHAT ASKED FOR IT: an upload in the list (its local number) or one
//! form (its own number). Its answer lands on that one and never on whatever form happens to be
//! open then (finding 1: a late answer used to close or rewrite a form opened since).
//!
//! CANCEL SURVIVES A SWITCH (finding 11): the worker sending the publish holds its own copy of the
//! server's address and the signer, and a [`Fate`] it shares with the page. When Cancel came
//! first, the worker takes the item (or the upload) off itself as soon as the publish is answered,
//! so a switch to another server or person in between, which forgets the page's own state, does
//! not leave it on Media. When the answer came first, the page takes it off at the press. Either
//! way "Cancelled" is said only once the taking off went (round 4, D4-2): the worker hands back
//! the undoing's own answer, and the page's take-off says nothing went when there is no signature
//! to send it with.
//!
//! ADDED TO AN ITEM (Media round 5, lane 1D, [`attach`]): an upload made from an item's Edit
//! window is added to that item (`POST /api/files/item-files/add`) instead of published, under the same
//! fate, so Cancel works the same; undoing it takes the file off the item by its number
//! (`POST /api/files/item-files/remove`), since a file on an item is not "waiting" and is never removed
//! by its upload name.
//!
//! ALREADY ON MEDIA IS DONE: the relay answers 409 "That upload is already on the Files page." to
//! a publish of a file that is (its owner check comes first, so it is the person's own). That is a
//! publish whose answer was lost to a quit or a switch and is now asked again: it counts as done.

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use super::add::Of;
use super::api::{self, PublishFields, Refusal, RemoveTarget, Signer};
use super::api_items::{file_add_request, file_number_of, file_remove_request, AttachTo};
use super::{signer_here, spawn, spawn_with, Job};
use crate::gui::GuiState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Open,
    /// Cancel came before the answer: the worker undoes it.
    CancelledFirst,
    Answered { published: bool },
    /// Cancel came after the answer: the page undid it.
    CancelledAfter,
}

/// Who undid a cancelled publish, if it was cancelled: what its answer means on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Undo {
    /// Not cancelled: the answer is the publish's own.
    None,
    /// The worker undid it, and the answer is what undoing it got (round 4, D4-2: "Cancelled" is
    /// said only when that went).
    Worker,
    /// The page undid it at the press (`take_off_now`), whose own answer says how it went.
    Page,
}

/// One publish's fate, shared by the page and the worker sending it.
#[derive(Debug, Clone)]
pub(crate) struct Fate {
    stage: Arc<Mutex<Stage>>,
    of: Of,
}

impl Fate {
    fn new(of: Of) -> Self {
        Fate { stage: Arc::new(Mutex::new(Stage::Open)), of }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Stage> {
        self.stage.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The worker's answer is in. True when Cancel came first: the worker takes it off.
    fn answered(&self, published: bool) -> bool {
        let mut stage = self.lock();
        if *stage == Stage::CancelledFirst {
            return true;
        }
        *stage = Stage::Answered { published };
        false
    }

    /// Cancel was pressed. Some(published) when the answer was already in, so the page takes it
    /// off now; None when the worker will, once it is answered.
    fn cancel(&self) -> Option<bool> {
        let mut stage = self.lock();
        match *stage {
            Stage::Answered { published } => {
                *stage = Stage::CancelledAfter;
                Some(published)
            }
            Stage::Open => {
                *stage = Stage::CancelledFirst;
                None
            }
            _ => None,
        }
    }

    pub(crate) fn undo(&self) -> Undo {
        match *self.lock() {
            Stage::CancelledFirst => Undo::Worker,
            Stage::CancelledAfter => Undo::Page,
            _ => Undo::None,
        }
    }
}

/// What a cancelled publish takes off: a published item always; a Media upload nobody wants any
/// more always; a shared file never, when it was not published (it stays where it was shared).
fn takes_off(of: &Of, published: bool) -> bool {
    published || *of != Of::Shared
}

/// The relay's 409 to a publish is "already on the Files page" for one's own file: done.
pub(crate) fn settle(result: Result<Value, Refusal>) -> Result<Value, Refusal> {
    match result {
        Err(r) if r.status == 409 => Ok(json!({ "ok": true, "already": true })),
        other => other,
    }
}

/// Send the publish of `fields` for `who` (an upload's local number or a form's number), signed
/// by `signer`, on a worker thread; its answer comes back as `Job::Publish`.
pub(crate) fn publish(state: &mut GuiState, of: Of, who: u64, signer: &Signer, fields: &PublishFields) {
    let req = api::publish_request(signer, fields);
    let fate = Fate::new(of.clone());
    let worker = fate.clone();
    let (base, signer) = (super::base_of(state), signer.clone());
    let filename = api::bare_name(&fields.filename).to_string();
    let name = filename.clone();
    state.files.publishes.push((who, fate));
    log::info!("Files: POST /api/files");
    let job = Job::Publish { of: of.clone(), filename, who };
    spawn_with(state, job, move || {
        let result = settle(api::send(&base, &req));
        if worker.answered(result.is_ok()) {
            // Cancel came first: this worker undoes it, and what goes back to the page is the
            // undoing's own answer, so "Cancelled" is said only when it went (round 4, D4-2).
            return if takes_off(&of, result.is_ok()) { take_off(&base, &signer, &name) } else { Ok(json!({ "ok": true })) };
        }
        result
    });
}

/// Add upload `local`'s file, up as `filename`, to the item `to` names as a file or its cover (Media
/// round 5, lane 1D): `POST /api/files/item-files/add`, signed by `signer`, on a worker thread, under
/// the same [`Fate`] as a publish, so its Cancel undoes it the same way. Its answer comes back as
/// `Job::Publish` with `Of::Attach`.
///
/// UNDOING AN ADDED FILE is not a publish's undoing: a file on an item is not "waiting", so the
/// relay will not remove it by its upload name. It is taken off the item by its number (the
/// answer's item names it, `file_number_of`) with `POST /api/files/item-files/remove`; an add that was
/// refused left the upload waiting, which goes by its name as before.
pub(crate) fn attach(state: &mut GuiState, local: u64, signer: &Signer, to: &AttachTo, filename: &str) {
    let item = to.item();
    let req = file_add_request(signer, &item, filename, to.role);
    let of = Of::Attach { item: item.clone(), role: to.role };
    let fate = Fate::new(of.clone());
    let worker = fate.clone();
    let (base, signer) = (super::base_of(state), signer.clone());
    let name = api::bare_name(filename).to_string();
    state.files.publishes.push((local, fate));
    log::info!("Files: POST /api/files/item-files/add");
    spawn_with(state, Job::Publish { of, filename: name.clone(), who: local }, move || {
        let result = api::send(&base, &req);
        if worker.answered(result.is_ok()) {
            // Cancel came first: take it off the item again (or, refused, the waiting upload).
            return match &result {
                Ok(v) => take_off_item(&base, &signer, &item, v, &name),
                Err(_) => take_off(&base, &signer, &name),
            };
        }
        result
    });
}

/// Take the file stored as `name` off `item` again, after the add answered `added` (its `item`
/// names the file's number). Blocking: on the worker.
fn take_off_item(base: &str, signer: &Signer, item: &api::ItemRef, added: &Value, name: &str) -> Result<Value, Refusal> {
    let Some(file) = added.get("item").and_then(api::Item::from_json).and_then(|i| file_number_of(&i, name)) else {
        return Err(Refusal::new(0, "The server's answer did not name the file, so it could not be taken off the item. Remove it with Edit on the item."));
    };
    let result = api::send(base, &file_remove_request(signer, item, file, name));
    if let Err(e) = &result {
        log::warn!("Files: a cancelled file add could not be taken off its item ({e}).");
    }
    result
}

/// What is said when an upload went onto its item: the item's title from the answer.
pub(crate) fn attached_words(v: &Value) -> String {
    match v.get("item").and_then(api::Item::from_json) {
        Some(i) => format!("Added to \"{}\".", i.display_title()),
        None => "Added to the item.".to_string(),
    }
}

/// Take `filename` (an upload, or the item made of it) off the server at `base`, as `signer`.
fn take_off(base: &str, signer: &Signer, filename: &str) -> Result<Value, Refusal> {
    let req = api::remove_request(signer, &RemoveTarget::Upload(filename.to_string()));
    let result = api::send(base, &req);
    if let Err(e) = &result {
        log::warn!("Files: a cancelled publish could not be taken off ({e}); an upload not on the Files page goes after a day.");
    }
    result
}

/// What is said when something on Media cannot be taken off for want of a signature.
pub(crate) const NO_SIGNATURE_TO_TAKE_OFF: &str = "Not taken off the server: there is no signature to send it with. Unlock your identity, then press Remove on it on the Files page.";

/// Take `filename` off the server now, from the page (a publish whose answer is already in). False,
/// with the reason said on the page, when there is no signature to send it with: round 3 sent
/// nothing then and still said "Cancelled" (the review's D4-2).
pub(crate) fn take_off_now(state: &mut GuiState, filename: &str) -> bool {
    let Some(signer) = signer_here(state) else {
        state.files.status = NO_SIGNATURE_TO_TAKE_OFF.to_string();
        return false;
    };
    let req = api::remove_request(&signer, &RemoveTarget::Upload(filename.to_string()));
    spawn(state, Job::Remove { id: None, from_admin: false }, req);
    true
}

/// Cancel the publish for `who`: undone by its worker when it is answered, or now, from here, when
/// it already was.
pub(crate) fn cancel(state: &mut GuiState, who: u64, filename: &str) {
    let Some(fate) = state.files.publishes.iter().find(|(w, _)| *w == who).map(|(_, f)| f.clone()) else { return };
    if let Some(published) = fate.cancel() {
        if matches!(fate.of, Of::Attach { .. }) && published {
            // Added to its item already, but its answer, which names the file's number on the
            // item, is not read yet: `published` takes it off the item when it is.
            return;
        }
        if takes_off(&fate.of, published) {
            take_off_now(state, filename);
        } else {
            state.files.status = cancelled_words(&fate.of).to_string();
        }
    }
}

/// The publish for `who` is answered: forget its fate, and say who undid it if it was cancelled.
pub(crate) fn finish(state: &mut GuiState, who: u64) -> Undo {
    let undo = state.files.publishes.iter().find(|(w, _)| *w == who).map(|(_, f)| f.undo()).unwrap_or(Undo::None);
    state.files.publishes.retain(|(w, _)| *w != who);
    undo
}

/// An add to `item` (upload `who`, stored as `filename`) is answered: the item as it is now goes
/// wherever the page shows it; a Cancel pressed after the add went takes the file off the item
/// again by its number, which only this answer names; then the upload's row is done with
/// (uploading.rs `published`).
fn attach_answered(state: &mut GuiState, item: &api::ItemRef, who: u64, filename: &str, result: Result<Value, Refusal>, undo: Undo) {
    let added = result.as_ref().ok().and_then(|v| v.get("item")).and_then(api::Item::from_json);
    match (undo, &added) {
        (Undo::None, Some(now)) => super::item_view::take_item(state, now.clone()),
        (Undo::Page, Some(now)) => match (file_number_of(now, filename), signer_here(state)) {
            (Some(file), Some(signer)) => {
                state.files.status = "Cancelling...".to_string();
                spawn(state, Job::FileRemove { id: item.id, file }, file_remove_request(&signer, item, file, filename));
            }
            (None, _) => state.files.status = not_taken_off_words("the server's answer did not name the file."),
            (_, None) => state.files.status = NO_SIGNATURE_TO_TAKE_OFF.to_string(),
        },
        // Refused: the upload waits on the server, and goes by its name.
        (Undo::Page, None) => {
            take_off_now(state, filename);
        }
        _ => {}
    }
    super::uploading::published(state, who, filename, result, undo);
}

/// What is said when a publish was cancelled and undone: a file uploaded for Media goes from the
/// server; a shared file was only kept off the Files page.
pub(crate) fn cancelled_words(of: &Of) -> &'static str {
    match of {
        Of::Shared => "Cancelled: it is not on the Files page.",
        Of::Upload | Of::Waiting | Of::Attach { .. } => super::uploading::CANCELLED_LINE,
    }
}

/// What is said when undoing a cancelled publish was refused or got no answer.
pub(crate) fn not_taken_off_words(why: &str) -> String {
    format!("Not taken off the server: {why} Remove it from the Files page, or from your uploads waiting to be put on it.")
}

/// The words for a publish that went: the item's title from the answer, when it has one.
pub(crate) fn on_media_words(v: &Value) -> String {
    match v.get("item").and_then(api::Item::from_json) {
        Some(i) => format!("\"{}\" is on the Files page now.", i.display_title()),
        None => "It is on the Files page now.".to_string(),
    }
}

/// A publish's answer (media.rs `on_done`): an upload's goes to the uploads list, a form's to that
/// form, if it is still the one open.
pub(crate) fn published(state: &mut GuiState, of: Of, who: u64, filename: &str, result: Result<Value, Refusal>) {
    let undo = finish(state, who);
    if let Of::Attach { item, .. } = &of {
        // An add is never "already on Media": its 409 is the relay's own words, read as they came.
        return attach_answered(state, item, who, filename, result, undo);
    }
    // Only a publish's own answer is "already on Media"; an undoing's is read as it came.
    let result = if undo == Undo::None { settle(result) } else { result };
    if of == Of::Upload {
        return super::uploading::published(state, who, filename, result, undo);
    }
    let mine = |state: &GuiState| state.files.form.as_ref().is_some_and(|f| f.id == who);
    match (undo, result) {
        (Undo::Worker, Ok(_)) => state.files.status = cancelled_words(&of).to_string(),
        (Undo::Worker, Err(e)) => state.files.status = not_taken_off_words(&e.words),
        // The page undid it at the press, and that undoing's own answer says how it went.
        (Undo::Page, _) => {}
        (Undo::None, Ok(v)) => {
            if mine(state) {
                state.files.form = None;
            }
            state.files.status = on_media_words(&v);
            super::reload(state);
            return;
        }
        (Undo::None, Err(e)) => match state.files.form.as_mut().filter(|f| f.id == who) {
            Some(f) => {
                f.sending = false;
                f.problem = if of == Of::Waiting { format!("Not put on the Files page: {} The file waits a day: press the button again.", e.words) } else { e.words };
            }
            None => state.files.status = e.words,
        },
    }
    super::refresh_mine(state);
}
