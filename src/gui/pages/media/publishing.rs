//! Putting a file that is on the server on the Media page (`POST /api/media`), and undoing it when
//! Cancel came while it was on its way (round 3 of docs/design/media-showcase.md, the 2026-10-11
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
//! ALREADY ON MEDIA IS DONE: the relay answers 409 "That upload is already on the Media page." to
//! a publish of a file that is (its owner check comes first, so it is the person's own). That is a
//! publish whose answer was lost to a quit or a switch and is now asked again: it counts as done.

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use super::add::Of;
use super::api::{self, PublishFields, Refusal, RemoveTarget, Signer};
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
fn takes_off(of: Of, published: bool) -> bool {
    published || of != Of::Shared
}

/// The relay's 409 to a publish is "already on the Media page" for one's own file: done.
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
    let fate = Fate::new(of);
    let worker = fate.clone();
    let (base, signer) = (super::base_of(state), signer.clone());
    let filename = api::bare_name(&fields.filename).to_string();
    let name = filename.clone();
    state.media.publishes.push((who, fate));
    log::info!("Media: POST /api/media");
    spawn_with(state, Job::Publish { of, filename, who }, move || {
        let result = settle(api::send(&base, &req));
        if worker.answered(result.is_ok()) {
            // Cancel came first: this worker undoes it, and what goes back to the page is the
            // undoing's own answer, so "Cancelled" is said only when it went (round 4, D4-2).
            return if takes_off(of, result.is_ok()) { take_off(&base, &signer, &name) } else { Ok(json!({ "ok": true })) };
        }
        result
    });
}

/// Take `filename` (an upload, or the item made of it) off the server at `base`, as `signer`.
fn take_off(base: &str, signer: &Signer, filename: &str) -> Result<Value, Refusal> {
    let req = api::remove_request(signer, &RemoveTarget::Upload(filename.to_string()));
    let result = api::send(base, &req);
    if let Err(e) = &result {
        log::warn!("Media: a cancelled publish could not be taken off ({e}); an upload not on Media goes after a day.");
    }
    result
}

/// What is said when something on Media cannot be taken off for want of a signature.
pub(crate) const NO_SIGNATURE_TO_TAKE_OFF: &str = "Not taken off the server: there is no signature to send it with. Unlock your identity, then press Remove on it on the Media page.";

/// Take `filename` off the server now, from the page (a publish whose answer is already in). False,
/// with the reason said on the page, when there is no signature to send it with: round 3 sent
/// nothing then and still said "Cancelled" (the review's D4-2).
pub(crate) fn take_off_now(state: &mut GuiState, filename: &str) -> bool {
    let Some(signer) = signer_here(state) else {
        state.media.status = NO_SIGNATURE_TO_TAKE_OFF.to_string();
        return false;
    };
    let req = api::remove_request(&signer, &RemoveTarget::Upload(filename.to_string()));
    spawn(state, Job::Remove { id: None, from_admin: false }, req);
    true
}

/// Cancel the publish for `who`: undone by its worker when it is answered, or now, from here, when
/// it already was.
pub(crate) fn cancel(state: &mut GuiState, who: u64, filename: &str) {
    let Some(fate) = state.media.publishes.iter().find(|(w, _)| *w == who).map(|(_, f)| f.clone()) else { return };
    if let Some(published) = fate.cancel() {
        if takes_off(fate.of, published) {
            take_off_now(state, filename);
        } else {
            state.media.status = cancelled_words(fate.of).to_string();
        }
    }
}

/// The publish for `who` is answered: forget its fate, and say who undid it if it was cancelled.
pub(crate) fn finish(state: &mut GuiState, who: u64) -> Undo {
    let undo = state.media.publishes.iter().find(|(w, _)| *w == who).map(|(_, f)| f.undo()).unwrap_or(Undo::None);
    state.media.publishes.retain(|(w, _)| *w != who);
    undo
}

/// What is said when a publish was cancelled and undone: a file uploaded for Media goes from the
/// server; a shared file was only kept off the Media page.
pub(crate) fn cancelled_words(of: Of) -> &'static str {
    match of {
        Of::Shared => "Cancelled: it is not on the Media page.",
        Of::Upload | Of::Waiting => super::uploading::CANCELLED_LINE,
    }
}

/// What is said when undoing a cancelled publish was refused or got no answer.
pub(crate) fn not_taken_off_words(why: &str) -> String {
    format!("Not taken off the server: {why} Remove it from the Media page, or from your uploads waiting to be put on it.")
}

/// The words for a publish that went: the item's title from the answer, when it has one.
pub(crate) fn on_media_words(v: &Value) -> String {
    match v.get("item").and_then(api::Item::from_json) {
        Some(i) => format!("\"{}\" is on the Media page now.", i.display_title()),
        None => "It is on the Media page now.".to_string(),
    }
}

/// A publish's answer (media.rs `on_done`): an upload's goes to the uploads list, a form's to that
/// form, if it is still the one open.
pub(crate) fn published(state: &mut GuiState, of: Of, who: u64, filename: &str, result: Result<Value, Refusal>) {
    let undo = finish(state, who);
    // Only a publish's own answer is "already on Media"; an undoing's is read as it came.
    let result = if undo == Undo::None { settle(result) } else { result };
    if of == Of::Upload {
        return super::uploading::published(state, who, filename, result, undo);
    }
    let mine = |state: &GuiState| state.media.form.as_ref().is_some_and(|f| f.id == who);
    match (undo, result) {
        (Undo::Worker, Ok(_)) => state.media.status = cancelled_words(of).to_string(),
        (Undo::Worker, Err(e)) => state.media.status = not_taken_off_words(&e.words),
        // The page undid it at the press, and that undoing's own answer says how it went.
        (Undo::Page, _) => {}
        (Undo::None, Ok(v)) => {
            if mine(state) {
                state.media.form = None;
            }
            state.media.status = on_media_words(&v);
            super::reload(state);
            return;
        }
        (Undo::None, Err(e)) => match state.media.form.as_mut().filter(|f| f.id == who) {
            Some(f) => {
                f.sending = false;
                f.problem = if of == Of::Waiting { format!("Not put on the Media page: {} The file waits a day: press the button again.", e.words) } else { e.words };
            }
            None => state.media.status = e.words,
        },
    }
    super::refresh_mine(state);
}
