//! The attach step: before a picked file leaves the device, what is hidden in it is shown, with one
//! switch (docs/design/media-showcase.md, "Files on the sender's device and in messages, exact",
//! parts 1 to 3; the operator's answers 4, 5 and 6 of 2026-10-11).
//!
//! WHEN IT SHOWS (the web's rule too): only when it has something to say or decide. A worker looks
//! at the file first (net/file_clean.rs `look_file`: a video by its box headers only, so a big one
//! is quick); a file with nothing hidden in it, that could be checked, and no line about the
//! recipient (a screenshot, a PDF) goes at once with the remembered switch.
//!
//! WHAT THE PERSON SEES, when it shows:
//! - the file's name and size ("Checking the file for hidden details..." only if the look takes
//!   more than a moment);
//! - what was found, in the same words as the web's attach step ("This photo says where it was
//!   taken and which camera took it.", then the details in plain words), or nothing when there is
//!   nothing;
//! - the switch, "Remove location and camera details", on by default and remembered as the last
//!   choice on this device (`FilesInMessages::remove_details`). The protected setup always removes
//!   and shows no switch, only a line saying so;
//! - for a file that could not be checked (answer 5): in a DM or a group, "Send it anyway?", and
//!   Send anyway marks its marker `unchecked`; in a public room, why it is not sent, and Send is
//!   off unless the switch is off (then it goes exactly as it is, with `keep_metadata=1`). Under
//!   the protected setup such a file is not sent anywhere, a DM included;
//! - in a DM, when the other person's published setting holds files from this sender (answer 6,
//!   their profile's `accepts_files`, asked for with `GET /api/members/{key}` when the DM is
//!   opened): "Ann accepts files only from friends. ..." The file still goes; their app asks them.
//!
//! The same look and switch are drawn by the Files page's upload and Add to Media
//! ([`draw_look`], [`Looking`]), so every upload path in the app says the same thing.
//!
//! Takes `use super::*` like the page's other children.

use super::*;
use super::attach_send::{AttachJob, Destination};
use crate::net::file_clean::{self, Look};
use crate::net::files_policy as policy;
use std::sync::mpsc::Receiver;

/// What a file says, worked out on a worker thread so a big file never stops the frame.
pub(crate) struct Looking {
    rx: Option<Receiver<Result<Look, String>>>,
    /// The answer, once it came.
    pub(crate) look: Option<Result<Look, String>>,
}

impl Looking {
    /// Look at the file at `path`, named `name`, on a worker thread.
    pub(crate) fn of_file(path: std::path::PathBuf, name: String) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(file_clean::look_file(&path, &name).map_err(|e| e.to_string()));
        });
        Looking { rx: Some(rx), look: None }
    }

    /// An answer already known (the tests, and a file looked at elsewhere).
    pub(crate) fn known(look: Result<Look, String>) -> Self {
        Looking { rx: None, look: Some(look) }
    }

    /// Take the worker's answer when it is in. True while still waiting.
    pub(crate) fn poll(&mut self) -> bool {
        if let Some(rx) = &self.rx {
            match rx.try_recv() {
                Ok(look) => {
                    self.look = Some(look);
                    self.rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return true,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.look = Some(Err("the check stopped".to_string()));
                    self.rx = None;
                }
            }
        }
        false
    }
}

/// What the look allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Verdict {
    /// The file may be sent now.
    pub can_send: bool,
    /// Sending it means "Send anyway": it could not be checked, the switch is on, and it goes to a
    /// private conversation (its marker then says `unchecked`).
    pub anyway: bool,
}

/// The verdict for a look, the switch, and where it goes (public: a server's room, Shared Files or
/// Media; private: a DM, a group, the scratchpad). Under the protected setup a file that could not
/// be checked is not sent anywhere, a DM included (the setup's promise is that location is always
/// removed; the web's rule too).
pub(crate) fn verdict(look: Option<&Result<Look, String>>, remove: bool, private: bool, protected: bool) -> Verdict {
    match look {
        Some(Ok(l)) if l.unchecked && remove => {
            let anyway = private && !protected;
            Verdict { can_send: anyway, anyway }
        }
        Some(Ok(_)) => Verdict { can_send: true, anyway: false },
        _ => Verdict { can_send: false, anyway: false },
    }
}

/// The switch as it starts: always on under the protected setup, else as last chosen here.
pub(crate) fn remove_at_start(state: &GuiState) -> bool {
    crate::engine::protected::is_on(state) || state.settings.files_in_messages.remove_details
}

/// Remember the switch as the person last set it (never under the protected setup, which decides
/// for them).
pub(crate) fn remember_switch(state: &mut GuiState, remove: bool) {
    if !crate::engine::protected::is_on(state) && state.settings.files_in_messages.remove_details != remove {
        state.settings.files_in_messages.remove_details = remove;
        state.settings_dirty = true;
    }
}

/// Draw what `looking` found, the switch (`remove`) and what a file that could not be checked
/// means where it is going. Returns what may be done.
pub(crate) fn draw_look(ui: &mut egui::Ui, theme: &Theme, looking: &mut Looking, remove: &mut bool, protected: bool, private: bool) -> Verdict {
    if looking.poll() {
        ui.label(RichText::new(policy::CHECKING).size(theme.font_size_small).color(theme.text_muted()));
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        return verdict(None, *remove, private, protected);
    }
    let look = match &looking.look {
        Some(Ok(l)) => l,
        Some(Err(e)) => {
            let line = format!("This file could not be read ({e}).");
            ui.label(RichText::new(line).size(theme.font_size_small).color(theme.danger()));
            return verdict(looking.look.as_ref(), *remove, private, protected);
        }
        None => return verdict(None, *remove, private, protected),
    };
    if protected {
        *remove = true;
    }
    if let Some(summary) = &look.description.summary {
        ui.label(RichText::new(summary).size(theme.font_size_body).color(theme.text_primary()));
        for d in &look.description.details {
            ui.label(RichText::new(format!("  · {}", d.words)).size(theme.font_size_small).color(theme.text_secondary()));
        }
    }
    let something = look.description.summary.is_some() || look.unchecked;
    if protected {
        if something {
            ui.label(RichText::new(policy::PROTECTED_ALWAYS).size(theme.font_size_small).color(theme.text_muted()));
        }
    } else if something {
        widgets::toggle(ui, theme, policy::SWITCH_LABEL, remove);
        if !*remove && look.description.summary.is_some() {
            ui.label(RichText::new(policy::KEPT_AS_IS).size(theme.font_size_small).color(theme.warning()));
        }
    }
    if look.unchecked && *remove {
        let (words, color) = if protected {
            (policy::PROTECTED_UNCHECKED, theme.danger())
        } else if private {
            (policy::SEND_ANYWAY, theme.warning())
        } else {
            (policy::NOT_SENT_PUBLIC, theme.danger())
        };
        ui.label(RichText::new(words).size(theme.font_size_small).color(color));
    }
    verdict(looking.look.as_ref(), *remove, private, protected)
}

/// A file picked with Attach, waiting in its attach step.
pub(crate) struct Step {
    pub job: AttachJob,
    pub looking: Looking,
    /// The switch.
    pub remove: bool,
    /// The conversation it was picked for: the step closes if the person leaves it.
    pub channel: String,
    /// When it was picked: the step shows "Checking..." only if the look takes a while.
    pub opened: std::time::Instant,
}

/// How long a look may take before the step shows "Checking..." (a quick one shows nothing).
const QUIET_LOOK: std::time::Duration = std::time::Duration::from_millis(600);

/// The chat's file state (`GuiState::files_flow`): the attach step, the people whose
/// `accepts_files` this app asked their server for, and this person's own profile as their server
/// holds it (for publishing their own choice without wiping anything; files_settings.rs).
#[derive(Default)]
pub struct FilesFlow {
    pub(crate) step: Option<Step>,
    /// (server, key) -> (their name, their word), as their server said.
    pub(crate) accepts: std::collections::HashMap<(String, String), (String, String)>,
    /// Who was asked already (answered or not), so nobody is asked twice on one server.
    asked: std::collections::HashSet<(String, String)>,
    /// The lookup on its way: (server, key) and the answer.
    asking: Option<((String, String), Receiver<Option<(String, String)>>)>,
    /// This person's own profile on the server they are on (files_settings.rs `sync_accepts`).
    pub(crate) own: crate::gui::pages::files_settings::OwnSync,
    /// The Settings > Data line after "Clear kept files".
    pub(crate) kept_note: String,
}

/// Is this a private conversation (a DM or a group) whose attach button the person turned off
/// (Settings > Privacy, "Show the attach button in private conversations")?
pub(crate) fn attach_off_here(state: &GuiState) -> bool {
    let private = matches!(Destination::of(&state.chat_active_channel), Destination::DirectMessage | Destination::Group);
    private && !state.settings.files_in_messages.attach_button
}

/// Open the attach step for `job`, the file at `path`: its look starts on a worker.
pub(super) fn open(state: &mut GuiState, job: AttachJob, path: &std::path::Path) {
    let looking = Looking::of_file(path.to_path_buf(), job.filename.clone());
    let remove = remove_at_start(state);
    let channel = state.chat_active_channel.clone();
    state.files_flow.step = Some(Step { job, looking, remove, channel, opened: std::time::Instant::now() });
}

/// The other person's published `accepts_files` and name, from their server's member answer
/// (`GET /api/members/{key}`, which says "friends" when they have no profile). None when it does
/// not say.
pub(crate) fn accepts_from_answer(answer: &serde_json::Value) -> Option<(String, String)> {
    let word = answer.get("accepts_files")?.as_str()?.to_string();
    let name = answer.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
    Some((name, word))
}

/// Ask this server for `peer`'s `accepts_files`, once per server and person, on a worker thread:
/// when their DM is opened, so the answer is in before a file is picked.
fn ask_accepts(state: &mut GuiState, peer: &str) {
    let server = norm_server_url(&state.server_url);
    let k = (server.clone(), peer.to_string());
    if server.is_empty() || peer.is_empty() || state.files_flow.asking.is_some() || !state.files_flow.asked.insert(k.clone()) {
        return;
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let url = format!("{}/api/members/{peer}", state.server_url.trim_end_matches('/'));
    std::thread::spawn(move || {
        let answer = ureq::get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .call()
            .ok()
            .and_then(|r| r.into_string().ok())
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
        let _ = tx.send(answer.as_ref().and_then(accepts_from_answer));
    });
    state.files_flow.asking = Some((k, rx));
}

/// Take a finished lookup.
fn poll_accepts(state: &mut GuiState) {
    let Some((k, rx)) = &state.files_flow.asking else { return };
    match rx.try_recv() {
        Ok(Some(found)) => {
            let k = k.clone();
            state.files_flow.accepts.insert(k, found);
            state.files_flow.asking = None;
        }
        Ok(None) | Err(std::sync::mpsc::TryRecvError::Disconnected) => state.files_flow.asking = None,
        Err(std::sync::mpsc::TryRecvError::Empty) => {}
    }
}

/// The sender's line for a DM to `peer`, from what their server said, when it applies.
pub(crate) fn sender_line_for(state: &GuiState, peer: &str) -> Option<String> {
    let (name, word) = state.files_flow.accepts.get(&(norm_server_url(&state.server_url), peer.to_string()))?;
    let name = if name.is_empty() { "They" } else { name.as_str() };
    policy::sender_line(name, word, crate::engine::protected::picture_friend(state, peer))
}

/// Whether the step has anything to say or decide: hidden details found, a file that could not be
/// checked, a line about the recipient, or a look that failed. A file with nothing in it (a
/// screenshot, a PDF) goes at once, the remembered switch applied (as on the web).
pub(crate) fn needs_asking(look: &Result<Look, String>, line: Option<&String>) -> bool {
    match look {
        Ok(l) => l.description.summary.is_some() || l.unchecked || line.is_some(),
        Err(_) => true,
    }
}

/// Send `step`'s file: the switch remembered, then the upload on its worker.
fn send_step(state: &mut GuiState, step: Step, anyway: bool) {
    remember_switch(state, step.remove);
    let mut job = step.job;
    job.clean = step.remove;
    job.allow_unchecked = anyway;
    attach_send::start(state, job);
}

/// The chat's part, every frame: a DM's `accepts_files` asked for when it is opened, this person's
/// own choice kept in step with their server (files_settings.rs), and the attach step, while one is
/// open: shown when it has something to say, else the file goes at once. Send starts the upload
/// (chat/attach_send.rs `start`) with the switch and the person's Send anyway.
pub(super) fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    poll_accepts(state);
    if let Some(peer) = state.chat_active_channel.strip_prefix("dm:").map(str::to_string) {
        ask_accepts(state, &peer);
    }
    crate::gui::pages::files_settings::sync_accepts(state);
    let Some(mut step) = state.files_flow.step.take() else { return };
    if step.channel != state.chat_active_channel {
        return; // the person left the conversation it was for: nothing is sent
    }
    let protected = crate::engine::protected::is_on(state);
    let private = step.job.to.encrypts_file();
    let line = step.channel.strip_prefix("dm:").and_then(|peer| sender_line_for(state, peer));
    if step.looking.poll() && step.opened.elapsed() < QUIET_LOOK {
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
        state.files_flow.step = Some(step);
        return;
    }
    if let Some(look) = &step.looking.look {
        if !needs_asking(look, line.as_ref()) {
            send_step(state, step, false); // nothing hidden in it: it goes as it is
            return;
        }
    }
    let mut open = true;
    let mut send = false;
    let mut cancel = false;
    let mut verdict_now = Verdict { can_send: false, anyway: false };
    widgets::dialog(ctx, theme, "chat_attach_step", "Send a file", &mut open, |ui| {
        ui.set_max_width(460.0);
        let size = super::attach_view::size_words(step.job.body.len());
        ui.label(RichText::new(format!("{} ({size})", step.job.filename)).size(theme.font_size_body).color(theme.text_primary()).strong());
        ui.add_space(theme.spacing_xs);
        verdict_now = draw_look(ui, theme, &mut step.looking, &mut step.remove, protected, private);
        if let Some(line) = &line {
            ui.add_space(theme.spacing_xs);
            ui.label(RichText::new(line).size(theme.font_size_small).color(theme.text_secondary()));
        }
        ui.add_space(theme.spacing_sm);
        ui.horizontal(|ui| {
            let words = if verdict_now.anyway { "Send anyway" } else { "Send" };
            ui.add_enabled_ui(verdict_now.can_send, |ui| {
                send = widgets::Button::primary(words).show(ui, theme);
            });
            cancel = widgets::Button::secondary("Cancel").show(ui, theme);
        });
    });
    if send && verdict_now.can_send {
        send_step(state, step, verdict_now.anyway);
        return;
    }
    if open && !cancel {
        state.files_flow.step = Some(step);
    }
}

#[cfg(test)]
#[path = "attach_step_tests.rs"]
mod tests;
