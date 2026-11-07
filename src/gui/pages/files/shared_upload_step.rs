//! The Files page's upload to the server's Shared Files, cleaned on this device first (2026-10-11,
//! docs/design/files-showcase.md, "Files on the sender's device and in messages, exact", parts 1
//! and 2). The same look and switch as the chat's attach step (chat/attach_step.rs `draw_look`):
//! what is hidden in the file, "Remove location and camera details" (remembered; always on under
//! the protected setup). A file that could not be checked is the person's to decide (2026-11-07,
//! BUG-264; the chat's question, BUG-260): its words, then "Don't send" first and focused and "Send
//! anyway" second (attach_step.rs `keep_answers`); "Send anyway" uploads it exactly as picked with
//! `keep_metadata=1` and the person's signed choice, as the switch turned off does. A file with
//! nothing hidden in it uploads at once. The file is read, cleaned and uploaded on a worker thread,
//! never on the frame.

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
    pub(crate) looking: Looking,
    pub remove: bool,
}

/// What the step came to this frame.
pub enum Outcome {
    /// Still waiting (for the look, or for the person).
    Waiting,
    /// Upload it: the upload is on its way, its answer in the receiver.
    Uploading(Receiver<Result<String, String>>),
    Cancelled,
    /// "Don't send", for a file that could not be checked: nothing was sent, and the page says so.
    NotSent,
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

/// Read, clean and upload the file to Shared Files: the worker's whole job. Kept with its details by
/// choice, it carries the person's signed choice for this very file (`signer`, BUG-260), or nothing
/// is sent.
pub(crate) fn upload(server: &str, token: &str, path: &std::path::Path, filename: &str, remove: bool, signer: Option<&crate::gui::pages::chat::attach_send::KeepSigner>) -> Result<String, String> {
    use crate::gui::pages::chat::attach_send;
    let (bytes, flags) = prepare(path, filename, remove)?;
    let mime = crate::gui::pages::chat::mime_for_filename(filename).to_string();
    let keep = if flags.keep_metadata { Some(signer.ok_or_else(attach_send::cannot_sign_words)?.choice(&bytes)) } else { None };
    attach_send::upload_with_poster_keep(server, token, filename, &mime, bytes, flags, None, keep.as_deref())
        .map(|url| format!("uploaded {url}"))
        .map_err(|(_, words)| words)
}

/// The upload on a worker: cleaned when `remove`, else exactly as picked (the switch off, or "Send
/// anyway"), with the signed choice.
fn spawn_upload(server: String, token: String, step: &UploadStep, remove: bool, signer: Option<crate::gui::pages::chat::attach_send::KeepSigner>) -> Receiver<Result<String, String>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let (path, filename) = (step.path.clone(), step.filename.clone());
    std::thread::spawn(move || {
        let _ = tx.send(upload(&server, &token, &path, &filename, remove, signer.as_ref()));
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
            return Outcome::Uploading(spawn_upload(server.to_string(), token.to_string(), step, step.remove, crate::gui::pages::chat::attach_send::KeepSigner::of(state)));
        }
    }
    let mut outcome = Outcome::Waiting;
    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new(&step.filename).color(theme.text_primary()).strong());
        let v = attach_step::draw_look(ui, theme, &mut step.looking, &mut step.remove, protected, false);
        ui.horizontal(|ui| {
            if v.anyway {
                // A file that could not be checked: the chat's two answers (BUG-264). "Send anyway"
                // sends it exactly as picked, signed; the switch stays as the person set it.
                let (no, yes) = attach_step::keep_answers(ui, theme, v.can_send);
                if yes {
                    outcome = Outcome::Uploading(spawn_upload(server.to_string(), token.to_string(), step, false, crate::gui::pages::chat::attach_send::KeepSigner::of(state)));
                } else if no {
                    outcome = Outcome::NotSent;
                }
                return;
            }
            let mut go = false;
            ui.add_enabled_ui(v.can_send, |ui| {
                go = widgets::Button::primary("Upload").show(ui, theme);
            });
            if go {
                outcome = Outcome::Uploading(spawn_upload(server.to_string(), token.to_string(), step, step.remove, crate::gui::pages::chat::attach_send::KeepSigner::of(state)));
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
