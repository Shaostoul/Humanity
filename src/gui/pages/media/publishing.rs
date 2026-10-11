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
//! not leave it on Media. When the answer came first, the page takes it off at the press.
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
    Cancelled,
    Answered { published: bool },
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
        if *stage == Stage::Cancelled {
            return true;
        }
        *stage = Stage::Answered { published };
        false
    }

    /// Cancel was pressed. Some(published) when the answer was already in, so the page takes it
    /// off now; None when the worker will, once it is answered.
    fn cancel(&self) -> Option<bool> {
        let mut stage = self.lock();
        let before = std::mem::replace(&mut *stage, Stage::Cancelled);
        match before {
            Stage::Answered { published } => Some(published),
            _ => None,
        }
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        *self.lock() == Stage::Cancelled
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
        if worker.answered(result.is_ok()) && takes_off(of, result.is_ok()) {
            take_off(&base, &signer, &name);
        }
        result
    });
}

/// Take `filename` (an upload, or the item made of it) off the server at `base`, as `signer`.
fn take_off(base: &str, signer: &Signer, filename: &str) {
    let req = api::remove_request(signer, &RemoveTarget::Upload(filename.to_string()));
    if let Err(e) = api::send(base, &req) {
        log::warn!("Media: a cancelled publish could not be taken off ({e}); an upload not on Media goes after a day.");
    }
}

/// Take `filename` off the server now, from the page (an upload whose publish was never sent, or
/// whose answer is already in).
pub(crate) fn take_off_now(state: &mut GuiState, filename: &str) {
    if let Some(signer) = signer_here(state) {
        let req = api::remove_request(&signer, &RemoveTarget::Upload(filename.to_string()));
        spawn(state, Job::Remove { id: None, from_admin: false }, req);
    }
}

/// Cancel the publish for `who`: undone by its worker when it is answered, or now, from here, when
/// it already was.
pub(crate) fn cancel(state: &mut GuiState, who: u64, filename: &str) {
    let Some(fate) = state.media.publishes.iter().find(|(w, _)| *w == who).map(|(_, f)| f.clone()) else { return };
    if let Some(published) = fate.cancel() {
        if takes_off(fate.of, published) {
            take_off_now(state, filename);
        }
    }
}

/// The publish for `who` is answered: forget its fate, and say whether it had been cancelled.
pub(crate) fn finish(state: &mut GuiState, who: u64) -> bool {
    let cancelled = state.media.publishes.iter().find(|(w, _)| *w == who).is_some_and(|(_, f)| f.is_cancelled());
    state.media.publishes.retain(|(w, _)| *w != who);
    cancelled
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
    let cancelled = finish(state, who);
    let result = settle(result);
    if of == Of::Upload {
        return super::uploading::published(state, who, filename, result, cancelled);
    }
    let mine = |state: &GuiState| state.media.form.as_ref().is_some_and(|f| f.id == who);
    if cancelled {
        state.media.status = super::uploading::CANCELLED_LINE.to_string();
    } else {
        match result {
            Ok(v) => {
                if mine(state) {
                    state.media.form = None;
                }
                state.media.status = on_media_words(&v);
                super::reload(state);
                return;
            }
            Err(e) => match state.media.form.as_mut().filter(|f| f.id == who) {
                Some(f) => {
                    f.sending = false;
                    f.problem = if of == Of::Waiting { format!("Not put on the Media page: {} The file waits a day: press the button again.", e.words) } else { e.words };
                }
                None => state.media.status = e.words,
            },
        }
    }
    super::refresh_mine(state);
}
