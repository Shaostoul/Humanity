//! The Files page's upload to the server's Shared Files, cleaned on this device first (2026-10-11,
//! docs/design/media-showcase.md, "Files on the sender's device and in messages, exact", parts 1
//! and 2). The same look and switch as the chat's attach step (chat/attach_step.rs `draw_look`):
//! what is hidden in the file, "Remove location and camera details" (remembered; always on under
//! the protected setup), and a file that could not be checked is not sent (Shared Files is public)
//! unless the person keeps everything by choice, which sends `keep_metadata=1`. A file with nothing
//! hidden in it uploads at once. The file is read, cleaned and uploaded on a worker thread, never on
//! the frame.

use egui::RichText;

use crate::gui::pages::chat::attach_step::{self, Looking};
use crate::gui::theme::Theme;
use crate::gui::{widgets, GuiState};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

/// A file picked for Shared Files, waiting for its look (and, when it has something to say, Upload).
pub struct UploadStep {
    pub path: PathBuf,
    pub filename: String,
    pub looking: Looking,
    pub remove: bool,
}

/// What the step came to this frame.
pub enum Outcome {
    /// Still waiting (for the look, or for the person).
    Waiting,
    /// Upload it: the upload is on its way, its answer in the receiver.
    Uploading(Receiver<Result<String, String>>),
    Cancelled,
}

/// The step for the file at `path`: its look starts on a worker.
pub(crate) fn start(state: &GuiState, path: PathBuf, filename: String) -> UploadStep {
    let looking = Looking::of_file(path.clone(), filename.clone());
    UploadStep { path, filename, looking, remove: attach_step::remove_at_start(state) }
}

/// What goes up for the file at `path`: read, cleaned (with `remove`), and the upload's flags
/// (share=1: it joins the public library; keep_metadata=1 only when kept by choice). Err, with
/// nothing to upload, for a file that could not be checked while details are to be removed.
pub(crate) fn prepare(path: &std::path::Path, filename: &str, remove: bool) -> Result<(Vec<u8>, crate::gui::pages::chat::attach_send::UploadFlags), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Read failed: {e}"))?;
    let bytes = if remove {
        let (cleaned, c) = crate::net::file_clean::clean_bytes(bytes, filename);
        if c.unchecked {
            return Err(format!("Not uploaded: {}", crate::gui::pages::chat::attach_send::NOT_CHECKED_PUBLIC));
        }
        cleaned
    } else {
        bytes
    };
    Ok((bytes, crate::gui::pages::chat::attach_send::UploadFlags { share: true, encrypted: false, keep_metadata: !remove }))
}

/// Read, clean and upload the file to Shared Files: the worker's whole job.
pub(crate) fn upload(server: &str, token: &str, path: &std::path::Path, filename: &str, remove: bool) -> Result<String, String> {
    let (bytes, flags) = prepare(path, filename, remove)?;
    let mime = crate::gui::pages::chat::mime_for_filename(filename).to_string();
    crate::gui::pages::chat::attach_send::upload_flagged(server, token, filename, &mime, bytes, flags).map(|url| format!("uploaded {url}"))
}

fn spawn_upload(server: String, token: String, step: &UploadStep) -> Receiver<Result<String, String>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let (path, filename, remove) = (step.path.clone(), step.filename.clone(), step.remove);
    std::thread::spawn(move || {
        let _ = tx.send(upload(&server, &token, &path, &filename, remove));
    });
    rx
}

/// Draw the step under the Shared Files list (only when it has something to say), and start the
/// upload on Upload, or at once for a file with nothing hidden in it.
pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, step: &mut UploadStep, server: &str, token: &str) -> Outcome {
    let protected = crate::engine::protected::is_on(state);
    if step.looking.poll() {
        ui.label(RichText::new(crate::net::files_policy::CHECKING).size(theme.font_size_small).color(theme.text_muted()));
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        return Outcome::Waiting;
    }
    if let Some(look) = &step.looking.look {
        if !attach_step::needs_asking(look, None) {
            attach_step::remember_switch(state, step.remove);
            return Outcome::Uploading(spawn_upload(server.to_string(), token.to_string(), step));
        }
    }
    let mut outcome = Outcome::Waiting;
    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new(&step.filename).color(theme.text_primary()).strong());
        let v = attach_step::draw_look(ui, theme, &mut step.looking, &mut step.remove, protected, false);
        ui.horizontal(|ui| {
            let mut go = false;
            ui.add_enabled_ui(v.can_send, |ui| {
                go = widgets::Button::primary("Upload").show(ui, theme);
            });
            if go {
                outcome = Outcome::Uploading(spawn_upload(server.to_string(), token.to_string(), step));
            }
            if widgets::Button::secondary("Cancel").show(ui, theme) {
                outcome = Outcome::Cancelled;
            }
        });
    });
    if matches!(outcome, Outcome::Uploading(_)) {
        attach_step::remember_switch(state, step.remove);
    }
    outcome
}

#[cfg(test)]
#[path = "shared_upload_step_tests.rs"]
mod tests;
