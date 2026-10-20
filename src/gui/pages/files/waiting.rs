//! On the Files page, under the uploads in progress: the person's own uploads waiting to be put on
//! Media (`/api/files/mine`'s `waiting`), with Put it on the Files page and Remove; and their unfinished
//! uploads the SERVER lists that this app has no record of (`uploads_in_progress`, the relay's
//! R3-7, round 3 of docs/design/files-showcase.md), with Cancel: one started on another device,
//! or one whose record was lost, which would otherwise hold part of the person's share for a day
//! with no way to see it. A row stays until the server confirmed its cancel (round 4, D4-2).

use egui::RichText;
use serde_json::Value;

use super::add::{Form, Source};
use super::api::{self, RemoveTarget, Waiting};
use super::uploading::{self, After};
use super::{spawn, Job};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

/// One of the person's own resumable uploads the server holds (`uploads_in_progress`, R3-7).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct InProgress {
    pub id: String,
    /// The file's own name, as the person's app sent it.
    pub filename: String,
    pub size: i64,
    /// How much of it the server has.
    pub offset: i64,
    /// When a piece last arrived (Unix ms): the server drops it a day after.
    pub touched_at: i64,
}

/// Read `uploads_in_progress: [{id, filename, size, offset, touched_at}]`.
pub fn parse_in_progress(v: Option<&Value>) -> Vec<InProgress> {
    let s = |o: &Value, k: &str| o.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let i = |o: &Value, k: &str| o.get(k).and_then(Value::as_i64).unwrap_or(0);
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|o| InProgress { id: s(o, "id"), filename: s(o, "filename"), size: i(o, "size"), offset: i(o, "offset"), touched_at: i(o, "touched_at") })
                .filter(|p| !p.id.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// The server's unfinished uploads this app is neither running nor offering to carry on.
pub(crate) fn unknown_sessions(state: &GuiState) -> Vec<InProgress> {
    let offered = uploading::offered(state);
    let Some(mine) = state.files.mine.as_ref() else { return Vec::new() };
    mine.in_progress
        .iter()
        .filter(|p| !state.files.uploads.iter().any(|e| e.record.id == p.id))
        .filter(|p| !offered.iter().any(|u| u.id == p.id))
        .cloned()
        .collect()
}

/// What is said above the server's unfinished uploads.
pub(crate) const SESSIONS_LINE: &str = "Started here before, or on another device, and not finished. Each holds part of your share until it is finished or cancelled; the server drops one a day after its last piece.";

/// The waiting uploads and the server's unfinished ones, if there are any.
pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    if !super::signed_in_here(state) {
        return;
    }
    let small = widgets::ButtonSize::Small;
    let sessions = unknown_sessions(state);
    if !sessions.is_empty() {
        widgets::subsection_label(ui, theme, "Your unfinished uploads on this server");
        widgets::body_hint(ui, theme, SESSIONS_LINE);
        for p in sessions {
            ui.horizontal_wrapped(|ui| {
                let shown = if p.filename.trim().is_empty() { "A file" } else { p.filename.trim() };
                ui.label(RichText::new(format!("{shown}: {} of {} arrived", api::size_words(p.offset), api::size_words(p.size))).size(theme.font_size_small).color(theme.text_primary()));
                // The row stays until the server confirmed the cancel (round 4, D4-2: round 3 took it
                // off at the press, even when the server answered 409 busy).
                if state.files.cancelling.contains(&p.id) {
                    ui.label(RichText::new("Cancelling...").size(theme.font_size_small).color(theme.text_muted()));
                } else if widgets::Button::secondary("Cancel").size(small).tooltip("The server drops what arrived.").show(ui, theme) {
                    uploading::cancel_on_server(state, &p.id);
                }
            });
        }
    }

    // What is being published now is not offered again here.
    let mut busy: Vec<String> = state
        .files
        .uploads
        .iter()
        .filter_map(|e| match &e.after {
            After::Publishing { filename } | After::NotPublished { filename, .. } => Some(filename.clone()),
            After::Uploading => None,
        })
        .collect();
    busy.extend(state.files.form.as_ref().and_then(|f| f.stored().map(str::to_string)));
    let waiting: Vec<Waiting> = state.files.mine.as_ref().map(|m| m.waiting.iter().filter(|w| !busy.contains(&w.filename)).cloned().collect()).unwrap_or_default();
    if waiting.is_empty() {
        return;
    }
    widgets::subsection_label(ui, theme, "Your uploads waiting to be put on the Files page");
    widgets::body_hint(ui, theme, "Each is deleted a day after it was uploaded, unless it is put on the Files page.");
    let sending = state.files.form.as_ref().is_some_and(|f| f.sending);
    for w in waiting {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("{}, {}", w.shown_name(), api::size_words(w.size_bytes))).size(theme.font_size_small).color(theme.text_primary()));
            if widgets::Button::secondary("Put it on the Files page").size(small).disabled(sending).show(ui, theme) {
                let source = Source::Waiting { filename: w.filename.clone(), name: w.shown_name().to_string(), len: w.size_bytes.max(0) as u64 };
                state.files.form = Some(Form::new(source));
            }
            if widgets::Button::danger("Remove").size(small).show(ui, theme) {
                state.files.confirm_waiting = Some(w.clone());
            }
        });
    }
}

/// The waiting upload's Remove confirmation: it is deleted from the server.
pub(crate) fn draw_confirm_waiting(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let Some(w) = state.files.confirm_waiting.clone() else { return };
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
        state.files.confirm_waiting = None;
    }
}
