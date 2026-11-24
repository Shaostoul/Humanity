//! Attach on a phone: Android's own document picker (2026-11-23; android/java/.../PickFiles.java,
//! android/README.md "The Java in the APK").
//!
//! WHY. The operator, 2026-11-23, on his OnePlus 9 Pro with v0.1571.0: "I can't upload images
//! because the windows aren't scaling to the phone." The app's own file browser (the desktop's
//! Attach window) opened in the app's private folder (/data/user/0/us.united_humanity.humanityos/
//! files), where none of his pictures are, and Android's scoped storage never lets an app browse the
//! phone's photos that way. Android's document picker (the Storage Access Framework's
//! ACTION_OPEN_DOCUMENT) lets the person choose from Photos, Downloads, Drive or anything else the
//! phone offers, several at once, and hands the app a content:// address for each, readable with no
//! permission of the app's own.
//!
//! HOW. Attach on Android opens the picker ([`open`]); the chat page asks each frame, and every 0.4 s
//! while it waits, whether the person has chosen ([`poll`]). The chosen documents are read on a
//! worker thread: each one's name and size from its provider, then its bytes, streamed by the Java
//! into a file in the app's own folder (`$TMPDIR/attach/`, inside it on a phone). A document over
//! the conversation's size cap (the same cap the desktop's picker is given, which hides bigger files
//! there) is not copied, and the person is told so in the chat's words for a file too large. Then the
//! copies go to the same place the desktop's picker sends its files ([`finish`], into
//! `attach_send::start_attach_all`): each opens its attach step, shows what is hidden in it, and is
//! sent as any file is. The copies are deleted when the next Attach starts with nothing still sending.
//!
//! A desktop keeps its own folder browser; the phone preview on a desktop shows that browser too
//! (only an Android build has Android's picker). The website uses the browser's own file picker.

use std::path::{Path, PathBuf};

use crate::gui::GuiState;

/// The picker's answer, as read from what `PickFiles.take` holds ([`parse`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// The chosen documents' content:// addresses, in the order the picker gave them.
    Picked(Vec<String>),
    /// The person closed the picker without choosing.
    Cancelled,
    /// The picker could not open, and why (for the person's notice and the log).
    Failed(String),
}

/// What `PickFiles.take` returned: "picked" then each chosen document's address, "cancelled", or
/// "failed" then why. Anything else is nothing to act on. Only content:// addresses are taken: that
/// is all Android's picker returns (Intent.ACTION_OPEN_DOCUMENT: "This must be a content:// URI"),
/// so anything else (a file:// path) is refused rather than read.
pub(crate) fn parse(raw: &[String]) -> Option<Outcome> {
    let (head, rest) = raw.split_first()?;
    match head.as_str() {
        "picked" => Some(Outcome::Picked(rest.iter().filter(|u| u.starts_with("content://")).cloned().collect())),
        "cancelled" => Some(Outcome::Cancelled),
        "failed" => Some(Outcome::Failed(rest.first().cloned().unwrap_or_default())),
        _ => None,
    }
}

/// The longest name a copy gets, in characters (its extension kept).
const NAME_MAX: usize = 120;

/// A file name for the copy of a document whose provider calls it `display`: no folder separators,
/// nothing a file name cannot hold, not hidden (no leading dot) and not empty, at most [`NAME_MAX`]
/// characters with its extension kept. A provider names its documents as it likes ("Display Name
/// ... is provider-specific, and might not necessarily be the filename", the guide), and a name like
/// `../x` must never leave the copy's folder.
pub(crate) fn safe_name(display: &str) -> String {
    let cleaned: String = display
        .chars()
        .map(|c| if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim().trim_start_matches('.').trim();
    if trimmed.is_empty() {
        return "file".to_string();
    }
    if trimmed.chars().count() <= NAME_MAX {
        return trimmed.to_string();
    }
    let (stem, ext) = match trimmed.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && ext.chars().count() <= 10 => (stem, format!(".{ext}")),
        _ => (trimmed, String::new()),
    };
    let keep = NAME_MAX.saturating_sub(ext.chars().count());
    format!("{}{ext}", stem.chars().take(keep).collect::<String>())
}

/// Whether a document of `size` bytes (`None`: its provider does not know) is copied for a
/// conversation whose cap is `limit`: when it is at most the cap, or unknown (the copy then stops at
/// the cap, and a document that turns out larger is refused as one known to be larger is).
pub(crate) fn fits(size: Option<u64>, limit: u64) -> bool {
    size.is_none_or(|n| n <= limit)
}

/// One chosen document, once the worker has read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Read {
    /// Copied into the app's folder, ready for the attach step.
    Ready(PathBuf),
    /// Over the conversation's cap: `len` bytes (at least the cap plus one when its provider did not
    /// say and the copy ran past the cap).
    TooLarge { name: String, len: u64 },
    /// Its provider would not give it up, and why.
    Failed { name: String, why: String },
}

/// The folder the copies go in: the app's own temporary folder (TMPDIR points inside the app's
/// private folder on a phone, src/android.rs).
pub(crate) fn copies_root() -> PathBuf {
    std::env::temp_dir().join("attach")
}

/// Where the `index`th document of a pick, named `name`, is copied: a folder of its own, so two
/// documents with the same name (two "image.jpg" from different places) never overwrite each other.
pub(crate) fn copy_path(root: &Path, index: usize, name: &str) -> PathBuf {
    root.join(index.to_string()).join(safe_name(name))
}

/// What Attach on the phone is doing (`FilesFlow::phone_pick`).
#[derive(Default)]
pub(crate) struct PhonePick {
    /// The picker is open: the conversation Attach was pressed in, and its size cap.
    pub(crate) waiting: Option<(String, u64)>,
    /// The chosen documents are being read: the conversation, the cap, and where the worker sends
    /// what it read.
    pub(crate) reading: Option<(String, u64, std::sync::mpsc::Receiver<Vec<Read>>)>,
}

/// The documents read for the conversation `channel` (cap `limit`) go where the desktop's picker
/// sends its files: the ready ones into the attach step one after another
/// (`attach_send::start_attach_all`), each one over the cap or unreadable said in a notice. Should
/// the person have left the conversation meanwhile, nothing is sent (a file chosen in a direct
/// message must never go anywhere else) and that is said.
pub(crate) fn finish(state: &mut GuiState, channel: &str, limit: u64, read: Vec<Read>) {
    let mut ready = Vec::new();
    for r in read {
        match r {
            Read::Ready(path) => ready.push(path),
            Read::TooLarge { name, len } => {
                let to = super::attach_send::Destination::of(channel);
                state.pending_notices.push(super::attach_send::too_large_line(&name, len, limit, to));
            }
            Read::Failed { name, why } => {
                log::warn!("Attach: {name} could not be read from its provider: {why}");
                state.pending_notices.push(humanity_core::i18n::tf("chat.attach_read_failed", &[("name", &name), ("why", &why)]));
            }
        }
    }
    if ready.is_empty() {
        return;
    }
    if state.chat_active_channel != channel {
        state.pending_notices.push(super::attach_send::left_before_the_rest(ready.len()));
        for path in &ready {
            let _ = std::fs::remove_file(path);
        }
        return;
    }
    super::attach_send::start_attach_all(state, ready);
}

/// Nothing chosen earlier is still on its way (no attach step open, nothing queued, no upload
/// running), so the copies kept for it may go.
fn idle(state: &GuiState) -> bool {
    state.files_flow.step.is_none() && state.files_flow.queue.is_none() && state.clipboard_upload.is_none()
}

/// Attach was pressed on the phone: open Android's picker for the conversation on screen, whose cap
/// is `limit`. The copies of an earlier pick are deleted first when nothing of it is still sending.
#[cfg(target_os = "android")]
pub(crate) fn open(state: &mut GuiState, limit: u64) {
    if idle(state) {
        let _ = std::fs::remove_dir_all(copies_root());
    }
    match humanity_core::platform::android_jni::pick_files_start() {
        Ok(()) => state.files_flow.phone_pick.waiting = Some((state.chat_active_channel.clone(), limit)),
        Err(why) => {
            log::warn!("Attach: Android's picker did not open: {why}");
            state.pending_notices.push(humanity_core::i18n::tf("chat.attach_picker_failed", &[("why", &why)]));
        }
    }
}

/// Every frame of the chat page (chat.rs `draw`): take the picker's answer when it has come, start
/// reading what was chosen, and hand what was read to the attach step. While the picker is open the
/// page asks again every 0.4 s (a phone draws only on change, engine/redraw.rs).
#[cfg(target_os = "android")]
pub(crate) fn poll(ctx: &egui::Context, state: &mut GuiState) {
    if let Some((channel, limit)) = state.files_flow.phone_pick.waiting.clone() {
        match humanity_core::platform::android_jni::pick_files_take() {
            Ok(Some(raw)) => {
                state.files_flow.phone_pick.waiting = None;
                match parse(&raw) {
                    Some(Outcome::Picked(uris)) if !uris.is_empty() => {
                        state.pending_notices.push(humanity_core::i18n::tn("chat.attach_reading", uris.len() as i64, &[]));
                        let (tx, rx) = std::sync::mpsc::channel();
                        let repaint = ctx.clone();
                        let spawned = std::thread::Builder::new().name("attach-read".into()).spawn(move || {
                            let _ = tx.send(read_all(&uris, limit));
                            repaint.request_repaint();
                        });
                        if spawned.is_ok() {
                            state.files_flow.phone_pick.reading = Some((channel, limit, rx));
                        }
                    }
                    Some(Outcome::Failed(why)) => {
                        log::warn!("Attach: Android's picker failed: {why}");
                        state.pending_notices.push(humanity_core::i18n::tf("chat.attach_picker_failed", &[("why", &why)]));
                    }
                    _ => {}
                }
            }
            Ok(None) => ctx.request_repaint_after(std::time::Duration::from_millis(400)),
            Err(why) => {
                state.files_flow.phone_pick.waiting = None;
                log::warn!("Attach: the picker's answer could not be read: {why}");
            }
        }
    }
    let done = match &state.files_flow.phone_pick.reading {
        Some((_, _, rx)) => match rx.try_recv() {
            Ok(read) => Some(read),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Vec::new()),
        },
        None => None,
    };
    if let Some(read) = done {
        if let Some((channel, limit, _)) = state.files_flow.phone_pick.reading.take() {
            finish(state, &channel, limit, read);
        }
    }
}

/// The worker's part: each chosen document's name and size from its provider, then its bytes
/// copied into the app's folder, streamed, stopping at `limit`.
#[cfg(target_os = "android")]
fn read_all(uris: &[String], limit: u64) -> Vec<Read> {
    use humanity_core::platform::android_jni as jni;
    let root = copies_root();
    uris.iter()
        .enumerate()
        .map(|(i, uri)| {
            let (name, size) = match jni::pick_files_describe(uri) {
                Ok((name, size)) => (safe_name(&name), size),
                Err(why) => return Read::Failed { name: safe_name(""), why },
            };
            if !fits(size, limit) {
                return Read::TooLarge { name, len: size.unwrap_or(limit + 1) };
            }
            let dest = copy_path(&root, i, &name);
            if let Some(dir) = dest.parent() {
                if let Err(e) = std::fs::create_dir_all(dir) {
                    return Read::Failed { name, why: e.to_string() };
                }
            }
            match jni::pick_files_copy(uri, &dest.to_string_lossy(), limit) {
                Ok(Some(_)) => Read::Ready(dest),
                Ok(None) => Read::TooLarge { name, len: size.unwrap_or(limit + 1) },
                Err(why) => {
                    let _ = std::fs::remove_file(&dest);
                    Read::Failed { name, why }
                }
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "attach_android_tests.rs"]
mod tests;
