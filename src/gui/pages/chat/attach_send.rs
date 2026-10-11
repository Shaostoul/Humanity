//! Sending a file into the chat: the Attach picker and a pasted image (10k of
//! docs/design/blocking-and-safe-mode.md, 2026-10-10).
//!
//! THE RULE: a file is only as private as the conversation if the FILE is encrypted, not just
//! the link to it. So anything sent into a direct message or a P2P group, attached or pasted,
//! is encrypted on this device with a fresh AES-256-GCM key (`dm_pq::encrypt_attachment`),
//! uploaded as ciphertext with `encrypted=1`, and sent as a `[[hum:file:v1]]` marker
//! (`dm_pq::build_file_marker`) carrying that key. The marker then travels inside the DM's seal
//! or inside the group's encrypted message, through `send_composed_content` like typed text.
//! The scratchpad, local-only by its label, is private too: encrypted the same way, its marker
//! kept only in the scratchpad on this device. A public channel keeps the plain upload: public
//! is public.
//!
//! Before this, only a picked file in a DM was encrypted (`is_dm: bool`); a pasted image there
//! and every file in a group went up as a plain public file. `Destination` replaces that flag so
//! the next kind of private conversation has to say which it is.
//!
//! The upload itself always runs on a worker thread; `run` takes the uploader as an argument so
//! the tests can see exactly what would be posted without a network.
//!
//! Takes `use super::*` like the page's other children.

use super::*;

/// Where a file is going, which decides whether the file itself is encrypted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Destination {
    /// A direct message (`dm:<key>`): the marker rides inside the DM's seal.
    DirectMessage,
    /// A P2P group (`p2pgroup:<id>`): the marker rides inside the group's encrypted message.
    Group,
    /// The scratchpad, the private note channel its label calls local-only: the file is
    /// encrypted like a DM's, and its marker (with the key) stays in the scratchpad on this
    /// device, which sends nothing (chat.rs `send_composed_content`). Before the 2026-10-10
    /// review it counted as public: the file went up as a plain public file, and a 3D model was
    /// even published to the server's Shared Files library.
    Scratchpad,
    /// A server channel, and anything else: the plain upload, as before.
    Public,
}

impl Destination {
    /// The destination of the chat view `channel` (`GuiState::chat_active_channel`).
    pub(crate) fn of(channel: &str) -> Self {
        if channel.starts_with("dm:") {
            Self::DirectMessage
        } else if channel.starts_with("p2pgroup:") {
            Self::Group
        } else if channel == "scratchpad" {
            Self::Scratchpad
        } else {
            Self::Public
        }
    }

    /// Whether the file is encrypted on this device before it leaves. No wildcard arm, so a new
    /// kind of conversation cannot be added without deciding this.
    pub(crate) fn encrypts_file(self) -> bool {
        match self {
            Self::DirectMessage | Self::Group | Self::Scratchpad => true,
            Self::Public => false,
        }
    }
}

/// One file waiting to be sent: everything the worker thread needs, taken from the app state
/// on the frame, so the worker never touches `GuiState`.
#[derive(Debug, Clone)]
pub(crate) struct AttachJob {
    pub server: String,
    /// This connection's upload token (`upload_token_here`).
    pub token: String,
    pub filename: String,
    pub mime: String,
    pub bytes: Vec<u8>,
    /// 3D model files also go to the server's public Shared Files library (`?share=1`), but
    /// only from a public channel: sharing a private file publicly would undo the encryption.
    pub share: bool,
    pub to: Destination,
    /// The most this job may upload: `limit_for` its destination.
    pub limit: u64,
}

/// The most a file sent to `to` may be: in a private conversation `ATTACH_MAX_BYTES` (the
/// receiving side decrypts in memory and refuses more); in a server's room, the person's own limit
/// there (`server_limit`, from the server), or `ATTACH_MAX_BYTES` until the server has said.
pub(crate) fn limit_for(server_limit: Option<u64>, to: Destination) -> u64 {
    if to.encrypts_file() {
        // Never over the person's own limit either, when the server allows them less.
        server_limit.map_or(ATTACH_MAX_BYTES, |mine| mine.min(ATTACH_MAX_BYTES))
    } else {
        server_limit.unwrap_or(ATTACH_MAX_BYTES)
    }
}

/// Keep this person's own upload limit from their entry of the server's `peer_list`
/// (`upload_limit_mb`, 2026-10-10). Another entry, or none, changes nothing.
pub(crate) fn note_upload_limit(state: &mut GuiState, peer_list: &serde_json::Value) {
    let me = state.profile_public_key.clone();
    let mine = peer_list
        .get("peers")
        .and_then(|p| p.as_array())
        .and_then(|peers| peers.iter().find(|p| p.get("public_key").and_then(|k| k.as_str()) == Some(me.as_str())));
    if let Some(mb) = mine.and_then(|p| p.get("upload_limit_mb")).and_then(|v| v.as_u64()) {
        state.upload_limit_bytes = Some(mb.saturating_mul(1024 * 1024));
    }
    // And this connection's upload token, kept with the server it is for.
    if let Some(token) = mine.and_then(|p| p.get("upload_token")).and_then(|v| v.as_str()).filter(|t| !t.is_empty()) {
        state.upload_token = Some((super::norm_server_url(&state.server_url), token.to_string()));
    }
}

/// The upload token for the server this person is on now, if it has sent one.
pub(crate) fn upload_token_here(state: &GuiState) -> Option<String> {
    let here = super::norm_server_url(&state.server_url);
    state.upload_token.as_ref().filter(|(server, _)| *server == here).map(|(_, t)| t.clone())
}

/// What is said when a file cannot go because this server has not signed this person in yet.
pub(crate) const NOT_SIGNED_IN_YET: &str = "The file was not sent: this server has not finished signing you in yet. Try again in a moment.";

/// The line shown when a file is over the limit for where it is going.
pub(crate) fn too_large_line(filename: &str, len: u64, limit: u64, to: Destination) -> String {
    let mb = |b: u64| b as f64 / (1024.0 * 1024.0);
    if to.encrypts_file() {
        format!(
            "{filename} is {:.1} MB. Files in a private conversation can be up to {} MB for now; a bigger one can go in a server's room, where your limit may be higher.",
            mb(len),
            limit / (1024 * 1024)
        )
    } else {
        format!("{filename} is {:.1} MB, over your {} MB limit on this server. An admin can raise it in Server Settings > Roles.", mb(len), limit / (1024 * 1024))
    }
}

/// Exactly what one POST to `/api/upload` carries. What `run` hands its uploader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Upload {
    pub filename: String,
    pub mime: String,
    pub bytes: Vec<u8>,
    pub share: bool,
    pub encrypted: bool,
}

/// The job for a file picked with Attach, sent to the conversation on screen. None, with
/// nothing sent, when it is over the size cap or its NAME holds the person's own recovery
/// phrase (step F's guard: the name travels inside the marker, where the guard on the message
/// text cannot read it, so it is checked here).
pub(crate) fn attach_job(state: &mut GuiState, filename: &str, bytes: Vec<u8>) -> Option<AttachJob> {
    let to = Destination::of(&state.chat_active_channel);
    let Some(token) = upload_token_here(state) else {
        state.ws_status = NOT_SIGNED_IN_YET.to_string();
        return None;
    };
    let limit = limit_for(state.upload_limit_bytes, to);
    if bytes.len() as u64 > limit {
        log::warn!("Attach rejected: {filename} is {} bytes (max {limit})", bytes.len());
        state.ws_status = too_large_line(filename, bytes.len() as u64, limit, to);
        return None;
    }
    if crate::engine::warnings::guard_stops(state, &[filename]) {
        return None;
    }
    let share = !to.encrypts_file()
        && crate::gui::widgets::file_browser::name_matches_ext(filename, SHARE_EXTS);
    Some(AttachJob {
        server: state.server_url.clone(),
        token,
        filename: filename.to_string(),
        mime: mime_for_filename(filename).to_string(),
        bytes,
        share,
        to,
        limit,
    })
}

/// The job for an image pasted from the clipboard (always a PNG the app encoded itself, under
/// a fixed name). The same size cap as Attach: a big screenshot is refused here rather than by
/// the server after the upload.
pub(crate) fn paste_job(state: &GuiState, png: Vec<u8>) -> Option<AttachJob> {
    let to = Destination::of(&state.chat_active_channel);
    let token = upload_token_here(state)?;
    let limit = limit_for(state.upload_limit_bytes, to);
    if png.len() as u64 > limit {
        log::warn!("Paste rejected: the image is {} bytes (max {limit})", png.len());
        return None;
    }
    Some(AttachJob {
        server: state.server_url.clone(),
        token,
        filename: "pasted-image.png".to_string(),
        mime: "image/png".to_string(),
        bytes: png,
        share: false,
        to,
        limit,
    })
}

/// Upload `job` with `upload(server, public_key, what)` and return the message text to send:
/// the `[[hum:file:v1]]` marker for a private conversation, the file's URL for a public one.
/// For a private one the uploader only ever sees ciphertext under a neutral name and type; the
/// real name and type ride inside the marker with the key. Runs on the upload worker.
pub(crate) fn run<U>(job: AttachJob, upload: U) -> Result<String, String>
where
    U: FnOnce(&str, &str, Upload) -> Result<String, String>,
{
    // Checked again here, so no path can reach an upload past the cap (a private conversation's
    // never past ATTACH_MAX_BYTES, whatever the job says).
    let limit = job.limit.min(limit_for(Some(job.limit), job.to));
    if job.bytes.len() as u64 > limit {
        return Err(format!("the file is {} bytes, over the {limit} limit", job.bytes.len()));
    }
    if !job.to.encrypts_file() {
        let plain = Upload { filename: job.filename, mime: job.mime, bytes: job.bytes, share: job.share, encrypted: false };
        return upload(&job.server, &job.token, plain);
    }
    let size = job.bytes.len() as u64;
    let (ciphertext, k, n) = crate::net::dm_pq::encrypt_attachment(&job.bytes)?;
    let sealed = Upload {
        filename: "attachment.enc".to_string(),
        mime: "application/octet-stream".to_string(),
        bytes: ciphertext,
        share: false,
        encrypted: true,
    };
    let url = upload(&job.server, &job.token, sealed)?;
    Ok(crate::net::dm_pq::build_file_marker(&crate::net::dm_pq::DmAttachment {
        url,
        k,
        n,
        name: job.filename,
        mime: job.mime,
        size,
    }))
}

/// Start sending `job` to the conversation on screen: the upload on a worker thread, its result
/// drained by `drain` on a later frame.
fn start(state: &mut GuiState, job: AttachJob) {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(run(job, |server, key, up| {
            upload_file_blocking_ext(server, key, &up.filename, &up.mime, up.bytes, up.share, up.encrypted)
        }));
    });
    state.clipboard_upload = Some((state.chat_active_channel.clone(), rx));
}

/// The Attach picker chose `path`: read it and start sending it.
pub(super) fn start_attach(state: &mut GuiState, path: &std::path::Path) {
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string();
    match std::fs::read(path) {
        Ok(bytes) => {
            if let Some(job) = attach_job(state, &filename, bytes) {
                start(state, job);
            }
        }
        Err(e) => log::warn!("Attach read failed for {filename}: {e}"),
    }
}

/// An image was pasted (Ctrl+V with a picture on the clipboard): start sending it.
pub(super) fn start_paste(state: &mut GuiState, png: Vec<u8>) {
    if let Some(job) = paste_job(state, png) {
        start(state, job);
    }
}

/// What is said when a private file's upload finishes after the person has left the
/// conversation it was for.
pub(crate) const LEFT_BEFORE_UPLOAD: &str =
    "The file was not sent, because you left the conversation before it finished uploading.";

/// Drain a finished upload: send its text through the single content authority
/// (`send_composed_content`), which seals a DM, encrypts a group message and keeps the
/// scratchpad local. (Before v0.708 this sent a raw chat message, which bypassed DM encryption:
/// the privacy-leak class the web client fixed in v0.698.2.) It goes to the view on screen,
/// like the web, except that a marker goes only to the conversation it was made for: its key
/// would let anyone in another view (a public channel, say) open the file. Nothing about the
/// upload is logged but whether it went, since a marker's text holds the file's key.
pub(super) fn drain(ctx: &egui::Context, state: &mut GuiState) {
    let Some((channel, rx)) = state.clipboard_upload.as_ref() else { return };
    let made_for = channel.clone();
    match rx.try_recv() {
        Ok(Ok(content)) => {
            state.clipboard_upload = None;
            let is_marker = crate::net::dm_pq::parse_file_marker(&content).is_some();
            if is_marker && made_for != state.chat_active_channel {
                state.pending_notices.push(LEFT_BEFORE_UPLOAD.to_string());
                return;
            }
            let sent = send_composed_content(state, &content);
            log::info!("Upload finished; routed send (sent={sent}, private={is_marker})");
        }
        Ok(Err(e)) => {
            state.clipboard_upload = None;
            log::warn!("Attachment upload failed: {e}");
            // Said to the person too (the server's own words when it refused), not only logged.
            state.pending_notices.push(format!("The file was not sent: {e}"));
        }
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            state.clipboard_upload = None;
            log::warn!("Attachment upload worker stopped unexpectedly");
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }
    }
}

/// Blocking multipart upload of any file to `<server_url>/api/upload?token=<this connection's token>`
/// (v0.708; `share=true` adds `&share=1` so 3D/model files publish to the
/// public Shared Files library, matching the web client). Returns the URL
/// from the JSON response. Runs on a worker thread at every call site, so
/// blocking here never freezes a frame. nginx caps the body at 6 MB.
pub(crate) fn upload_file_blocking(
    server_url: &str,
    token: &str,
    filename: &str,
    mime: &str,
    bytes: Vec<u8>,
    share: bool,
) -> Result<String, String> {
    upload_file_blocking_ext(server_url, token, filename, mime, bytes, share, false)
}

/// As `upload_file_blocking`, plus an `encrypted` flag. When set, the body is
/// opaque ciphertext and the server skips format/EXIF handling
/// (`?encrypted=1`). Used for files in private conversations.
pub(crate) fn upload_file_blocking_ext(
    server_url: &str,
    token: &str,
    filename: &str,
    mime: &str,
    bytes: Vec<u8>,
    share: bool,
    encrypted: bool,
) -> Result<String, String> {
    let base = server_url.trim_end_matches('/');
    let share_q = if share { "&share=1" } else { "" };
    let enc_q = if encrypted { "&encrypted=1" } else { "" };
    let upload_url = format!("{base}/api/upload?token={token}{share_q}{enc_q}", base = base, token = token);
    let boundary = format!("HumanityOSBoundary{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    // Sanitize the filename for the multipart header (quotes/CRLF would
    // corrupt the form-data framing).
    let safe_name: String = filename
        .chars()
        .map(|c| if c == '"' || c == '\r' || c == '\n' { '_' } else { c })
        .collect();
    let preamble = format!(
        "--{b}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{f}\"\r\nContent-Type: {m}\r\n\r\n",
        b = boundary, f = safe_name, m = mime,
    );
    let epilogue = format!("\r\n--{b}--\r\n", b = boundary);
    let mut body: Vec<u8> = Vec::with_capacity(preamble.len() + bytes.len() + epilogue.len());
    body.extend_from_slice(preamble.as_bytes());
    body.extend_from_slice(&bytes);
    body.extend_from_slice(epilogue.as_bytes());

    let resp = ureq::post(&upload_url)
        .set("Content-Type", &format!("multipart/form-data; boundary={}", boundary))
        .send_bytes(&body)
        .map_err(upload_error_words)?;
    let body_str = resp.into_string()
        .map_err(|e| format!("read response: {e}"))?;
    let val: serde_json::Value = serde_json::from_str(&body_str)
        .map_err(|e| format!("parse JSON: {e}; body={body_str}"))?;
    val.get("url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("response missing 'url' field: {body_str}"))
}

/// What a failed upload says to the person: the server's own words when it refused (its body:
/// "File too large ... your limit on this server is N MB", "Your role isn't allowed ..."), or that
/// the server could not be reached.
pub(crate) fn upload_error_words(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(code, resp) => {
            let text = resp.into_string().unwrap_or_default();
            let text = text.trim();
            if text.is_empty() || text.starts_with('<') {
                format!("the server refused it ({code}).")
            } else {
                text.to_string()
            }
        }
        other => format!("the server could not be reached ({other})."),
    }
}

#[cfg(test)]
#[path = "attach_send_tests.rs"]
mod tests;
