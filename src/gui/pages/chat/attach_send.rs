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
//! the tests can see exactly what would be posted without a network. A picked file is read there
//! too, never on the frame, and kept to one copy in memory (chat/attach_body.rs, round 3 of
//! docs/design/files-showcase.md).
//!
//! Takes `use super::*` like the page's other children.

use super::*;
use super::attach_body::{self, Body};

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
    /// The file: its bytes (a paste), or its path, read by the worker.
    pub body: Body,
    /// 3D model files also go to the server's public Shared Files library (`?share=1`), but
    /// only from a public channel: sharing a private file publicly would undo the encryption.
    pub share: bool,
    pub to: Destination,
    /// The most this job may upload: `limit_for` its destination.
    pub limit: u64,
    /// Remove location and camera details before it leaves (the attach step's switch, 2026-10-11).
    /// Off: sent exactly as it is, with `keep_metadata=1` on a public upload.
    pub clean: bool,
    /// The person said Send anyway to a file that could not be checked (private conversations
    /// only): its marker then says `unchecked`. Without it such a file is not sent.
    pub allow_unchecked: bool,
}

/// What signs the person's choice to send a file with its hidden details (2026-11-06, BUG-260):
/// their seed and identity key, taken from the app state on the frame so an upload's worker can
/// sign without touching `GuiState`. The relay keeps a file's details only on this signature over
/// that very file (relay/upload_rules.rs `KeepChoice`): an upload token alone is not enough.
#[derive(Clone, PartialEq, Eq)]
pub struct KeepSigner {
    seed: Vec<u8>,
    me: String,
}

impl std::fmt::Debug for KeepSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeepSigner").field("me", &self.me).finish_non_exhaustive() // never the seed
    }
}

impl KeepSigner {
    /// The signer for the identity signed in now; None while it is locked.
    pub(crate) fn of(state: &GuiState) -> Option<Self> {
        let seed = state.private_key_bytes.clone()?;
        (!state.profile_public_key.is_empty()).then(|| KeepSigner { seed, me: state.profile_public_key.clone() })
    }

    /// The `keep` choice (its JSON) for the file whose BLAKE3 is `blake3_hex`, signed now: Dilithium3
    /// by this identity over `hum/upload-keep/v1\n{key}\n{blake3}\n{ms}` (the relay's `keep_text`).
    pub(crate) fn choice_for_hash(&self, blake3_hex: &str) -> String {
        use base64::{engine::general_purpose::STANDARD as B64, Engine};
        use humanity_protocol::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};
        let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
        let kp = DilithiumKeypair::from_seed(&derive_dilithium_seed(&self.seed));
        let sig = kp.sign(crate::relay::upload_rules::keep_text(&self.me, blake3_hex, ts).as_bytes());
        serde_json::json!({ "ts": ts, "sig": B64.encode(sig) }).to_string()
    }

    /// [`Self::choice_for_hash`] for `bytes`.
    pub(crate) fn choice(&self, bytes: &[u8]) -> String {
        self.choice_for_hash(&crate::relay::upload_rules::blake3_hex(bytes))
    }
}

/// What is said when a file to be sent with its hidden details cannot be, because the identity is
/// locked and the choice cannot be signed.
pub(crate) fn cannot_sign_words() -> String {
    humanity_core::i18n::t("upload_keep.cannot_sign").to_string()
}

/// The largest private file any server can allow, in bytes: the top of the range the relay keeps
/// a role's `max_private_file_mb` in, read through the relay's own `with_limits_in_range` (the
/// module naming the number is the relay's own). It is also what the receiving side will open
/// (chat/attach_view.rs `max_ciphertext`): no path sends past it whatever a job says.
pub(crate) fn private_ceiling() -> u64 {
    let top = crate::relay::storage::RoleDef { max_private_file_mb: i64::MAX, ..Default::default() }.with_limits_in_range();
    top.max_private_file_mb.max(1) as u64 * 1024 * 1024
}

/// The person's own upload limits on the server they are on, from their own entry of its peer
/// list: `room` (`upload_limit_mb`, a file in a server's room) and `private` (`private_limit_mb`,
/// a file in a private conversation, 2026-10-11). Each None until the server has said.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Limits {
    pub room: Option<u64>,
    pub private: Option<u64>,
}

impl Limits {
    /// This person's limits on the server they are on now: what that server told this identity,
    /// nothing (the fallback) when it told them to someone else or about another server.
    pub(crate) fn of(state: &GuiState) -> Self {
        let l = &state.upload_limits;
        if l.server != super::norm_server_url(&state.server_url) || l.key != state.profile_public_key {
            return Limits::default();
        }
        Limits { room: l.room, private: l.private }
    }
}

/// The upload limits a server told this person (`GuiState::upload_limits`), with the server and
/// the identity they were told to, so they never outlive either (round 3 of
/// docs/design/files-showcase.md, the review's finding 10: the private-file limit of one server
/// was kept after a switch to another, or after a restore from a recovery phrase).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UploadLimits {
    /// The server (normalised) and the identity (public key) they were told to.
    pub server: String,
    pub key: String,
    /// A file in a server's room (`upload_limit_mb`), in bytes.
    pub room: Option<u64>,
    /// A file in a private conversation (`private_limit_mb`), in bytes.
    pub private: Option<u64>,
}

/// This person's own limit for one file in a room of the server they are on, if it has said.
pub(crate) fn room_limit_here(state: &GuiState) -> Option<u64> {
    Limits::of(state).room
}

/// The most a file sent to `to` may be: in a private conversation the person's own private-file
/// limit on this server (the relay holds an encrypted upload to it, not to the room limit); in a
/// server's room, their own limit there. Either is `ATTACH_MAX_BYTES` until the server has said.
pub(crate) fn limit_for(limits: Limits, to: Destination) -> u64 {
    if to.encrypts_file() {
        limits.private.unwrap_or(ATTACH_MAX_BYTES).min(private_ceiling())
    } else {
        limits.room.unwrap_or(ATTACH_MAX_BYTES)
    }
}

/// Keep this person's own upload limits from their entry of the server's `peer_list`
/// (`upload_limit_mb`, 2026-10-10; `private_limit_mb`, 2026-10-11), with the server and identity
/// they are for: limits told to another server or identity before are dropped first. Another
/// entry, or none, changes nothing.
pub(crate) fn note_upload_limit(state: &mut GuiState, peer_list: &serde_json::Value) {
    let me = state.profile_public_key.clone();
    let here = super::norm_server_url(&state.server_url);
    if state.upload_limits.server != here || state.upload_limits.key != me {
        state.upload_limits = UploadLimits { server: here, key: me.clone(), room: None, private: None };
    }
    let mine = peer_list
        .get("peers")
        .and_then(|p| p.as_array())
        .and_then(|peers| peers.iter().find(|p| p.get("public_key").and_then(|k| k.as_str()) == Some(me.as_str())));
    let mb_at = |field: &str| mine.and_then(|p| p.get(field)).and_then(|v| v.as_u64()).map(|mb| mb.saturating_mul(1024 * 1024));
    if let Some(bytes) = mb_at("upload_limit_mb") {
        state.upload_limits.room = Some(bytes);
    }
    if let Some(bytes) = mb_at("private_limit_mb") {
        state.upload_limits.private = Some(bytes);
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
            "{filename} is {:.1} MB. Files in a private conversation can be up to {} MB for you on this server; an admin can raise it in Server Settings > Roles.",
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
    /// The person chose to keep the file's hidden details (`keep_metadata=1`, a public upload only).
    pub keep_metadata: bool,
    /// A video's poster picture (a JPEG this app made, chat/video_poster.rs), sent as the form's
    /// `poster` part before the file (2026-11-06): a public video only, never a private file.
    pub poster: Option<Vec<u8>>,
}

/// The job for a file picked with Attach, sent to the conversation on screen. None, with
/// nothing sent, when it is over the size cap or its NAME holds the person's own recovery
/// phrase (step F's guard: the name travels inside the marker, where the guard on the message
/// text cannot read it, so it is checked here).
#[cfg(test)]
pub(crate) fn attach_job(state: &mut GuiState, filename: &str, bytes: Vec<u8>) -> Option<AttachJob> {
    job_for(state, filename, Body::Bytes(bytes))
}

/// The job for the file at `path`, picked with Attach: its size is read now (the cap is checked
/// before anything is sent), its bytes only by the worker (round 3, the review's finding 8: a
/// gigabyte read on the frame stopped the app drawing until it was read).
pub(crate) fn attach_file_job(state: &mut GuiState, path: &std::path::Path) -> Option<AttachJob> {
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string();
    match std::fs::metadata(path) {
        Ok(m) => job_for(state, &filename, Body::File { path: path.to_path_buf(), len: m.len() }),
        Err(e) => {
            log::warn!("Attach read failed for {filename}: {e}");
            state.pending_notices.push(format!("The file was not sent: {filename} could not be read ({e})."));
            None
        }
    }
}

/// [`attach_job`] for any `body`.
fn job_for(state: &mut GuiState, filename: &str, body: Body) -> Option<AttachJob> {
    let to = Destination::of(&state.chat_active_channel);
    let Some(token) = upload_token_here(state) else {
        state.ws_status = NOT_SIGNED_IN_YET.to_string();
        return None;
    };
    let limit = limit_for(Limits::of(state), to);
    if body.len() > limit {
        log::warn!("Attach rejected: {filename} is {} bytes (max {limit})", body.len());
        state.ws_status = too_large_line(filename, body.len(), limit, to);
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
        body,
        share,
        to,
        limit,
        clean: true,
        allow_unchecked: false,
    })
}

/// The job for an image pasted from the clipboard (always a PNG the app encoded itself, under
/// a fixed name). The same size cap as Attach: a big screenshot is refused here rather than by
/// the server after the upload.
pub(crate) fn paste_job(state: &GuiState, png: Vec<u8>) -> Option<AttachJob> {
    let to = Destination::of(&state.chat_active_channel);
    let token = upload_token_here(state)?;
    let limit = limit_for(Limits::of(state), to);
    if png.len() as u64 > limit {
        log::warn!("Paste rejected: the image is {} bytes (max {limit})", png.len());
        return None;
    }
    Some(AttachJob {
        server: state.server_url.clone(),
        token,
        filename: "pasted-image.png".to_string(),
        mime: "image/png".to_string(),
        body: Body::Bytes(png),
        share: false,
        to,
        limit,
        // A pasted picture is a PNG this app encoded itself, with nothing hidden in it; it is still
        // cleaned, which changes nothing.
        clean: true,
        allow_unchecked: false,
    })
}

/// Upload `job` with `upload(server, public_key, what)` and return the message text to send:
/// the `[[hum:file:v1]]` marker for a private conversation, the file's URL for a public one.
/// For a private one the uploader only ever sees ciphertext under a neutral name and type; the
/// real name and type ride inside the marker with the key. Runs on the upload worker, which reads
/// a picked file here and encrypts it in its own buffer: one copy of the file in memory.
pub(crate) fn run<U>(job: AttachJob, upload: U) -> Result<String, String>
where
    U: FnOnce(&str, &str, Upload) -> Result<String, String>,
{
    // Checked again here, so no path can reach an upload past the cap (a private conversation's
    // never past `private_ceiling`, what the receiving side opens, whatever the job says).
    let limit = if job.to.encrypts_file() { job.limit.min(private_ceiling()) } else { job.limit };
    if job.body.len() > limit {
        return Err(format!("the file is {} bytes, over the {limit} limit", job.body.len()));
    }
    // A video going to a public place gets its poster picture, made here on the worker from the
    // file itself (docs/design/media-player.md, "Rung 2: the poster picture"). Never for a private
    // conversation: a plain picture of a sealed file, at an address anyone can fetch, would show what
    // it holds. A video nothing here decodes goes without one.
    let poster = if job.to.encrypts_file() || !super::video_poster::is_video_name(&job.filename) { None } else { poster_of(&job.body) };
    let mut bytes = attach_body::read(job.body, limit)?;
    // Part 1 of the files section of docs/design/files-showcase.md: the file is cleaned HERE, on the
    // sender's device, before it is encrypted or uploaded, so a DM's or a group's file (which the
    // server only ever sees sealed) leaves without where it was taken too.
    let mut unchecked = false;
    // A public file that could not be cleaned goes only on the person's "Send anyway" (2026-11-06,
    // BUG-260), exactly as it is, with their signed choice to keep its details (`keep_metadata`).
    let mut keep_anyway = false;
    if job.clean {
        let (cleaned, c) = crate::net::file_clean::clean_bytes(bytes, &job.filename);
        bytes = cleaned;
        if c.unchecked {
            if !job.to.encrypts_file() {
                if !job.allow_unchecked {
                    return Err(NOT_CHECKED_PUBLIC.to_string());
                }
                keep_anyway = true;
            }
            if !job.allow_unchecked {
                return Err(NOT_CHECKED.to_string());
            }
            unchecked = true;
        }
    }
    if !job.to.encrypts_file() {
        let plain = Upload { filename: job.filename, mime: job.mime, bytes, share: job.share, encrypted: false, keep_metadata: !job.clean || keep_anyway, poster };
        return upload(&job.server, &job.token, plain);
    }
    let size = bytes.len() as u64;
    let (k, n) = crate::net::dm_pq::encrypt_attachment_in_place(&mut bytes)?;
    let sealed = Upload {
        filename: "attachment.enc".to_string(),
        mime: "application/octet-stream".to_string(),
        bytes, // now the ciphertext, in the same buffer
        share: false,
        encrypted: true,
        keep_metadata: false,
        poster: None,
    };
    let url = upload(&job.server, &job.token, sealed)?;
    let att = crate::net::dm_pq::DmAttachment { url, k, n, name: job.filename, mime: job.mime, size };
    Ok(if unchecked { crate::net::dm_pq::build_file_marker_unchecked(&att) } else { crate::net::dm_pq::build_file_marker(&att) })
}

/// The poster picture of the video in `body` (chat/video_poster.rs `make_poster`), or None (logged)
/// when nothing here decodes it. A picked file is read where it is; bytes already in memory are
/// written to a temporary file first (the player opens a file), removed once it is read.
fn poster_of(body: &Body) -> Option<Vec<u8>> {
    let made = match body {
        Body::File { path, .. } => super::video_poster::make_poster(path),
        Body::Bytes(bytes) => {
            let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
            let tmp = std::env::temp_dir().join(format!("hos-poster-{}-{stamp}.video", std::process::id()));
            let made = std::fs::write(&tmp, bytes).map_err(|e| e.to_string()).and_then(|_| super::video_poster::make_poster(&tmp));
            let _ = std::fs::remove_file(&tmp);
            made
        }
    };
    made.map_err(|e| log::info!("No poster picture for this video: {e}")).ok()
}

/// Why a file that could not be checked was not sent to a private conversation: the person did not
/// say Send anyway (the attach step asks).
pub(crate) const NOT_CHECKED: &str = "it could not be read all the way through, so where it was taken may still be in it.";
/// Why one was not sent to a public place (the server would refuse it too).
pub(crate) const NOT_CHECKED_PUBLIC: &str =
    "it could not be read all the way through, so where it was taken may still be in it, and a public place does not take such a file.";

/// Start sending `job` to the conversation on screen: the upload on a worker thread, its result
/// drained by `drain` on a later frame.
pub(super) fn start(state: &mut GuiState, job: AttachJob) {
    let (tx, rx) = std::sync::mpsc::channel();
    let signer = KeepSigner::of(state);
    // A public file sent as cleaned is kept here until its answer: if the server could not remove
    // its hidden details after all (its 422, BUG-260), the person is asked, and "Send anyway" sends
    // this job again exactly as it is (`take_result`, attach_step.rs `draw_server_ask`).
    state.files_flow.sent_public = (!job.to.encrypts_file() && job.clean && !job.allow_unchecked).then(|| job.clone());
    std::thread::spawn(move || {
        let _ = tx.send(run(job, |server, key, up| {
            let flags = UploadFlags { share: up.share, encrypted: up.encrypted, keep_metadata: up.keep_metadata };
            // Kept with its details by choice: the person's signed choice for this very file goes
            // with it, or the server stores nothing (BUG-260).
            let keep = if up.keep_metadata {
                Some(signer.as_ref().ok_or_else(cannot_sign_words)?.choice(&up.bytes))
            } else {
                None
            };
            upload_with_poster_keep(server, key, &up.filename, &up.mime, up.bytes, flags, up.poster, keep.as_deref())
                .map_err(|(status, words)| if status == Some(CANNOT_CLEAN_STATUS) { format!("{CANNOT_CLEAN_TAG}{words}") } else { words })
        }));
    });
    state.clipboard_upload = Some((state.chat_active_channel.clone(), rx));
}

/// An upload's answer that means "its hidden details could not be removed" (the relay's
/// `CANNOT_CLEAN`, BUG-260).
pub(crate) const CANNOT_CLEAN_STATUS: u16 = 422;

/// Marks a failed chat upload's words as the server's "could not be cleaned" on their way from the
/// worker to `drain`, which asks the person instead of showing them (never shown itself).
pub(crate) const CANNOT_CLEAN_TAG: &str = "\u{1}cannot-clean\u{1}";

/// The Attach picker chose `path`: its attach step opens (chat/attach_step.rs), which shows what is
/// hidden in it and sends it on Send (the worker reads it).
pub(super) fn start_attach(state: &mut GuiState, path: &std::path::Path) {
    // A DM file over the person's private file limit goes straight to the other person's device
    // instead of being refused (device-to-device big files, files-showcase.md answer 10).
    match super::attach_step::route_pick(state, path) {
        super::attach_step::Picked::Direct(job, direct) => super::attach_step::open_direct(state, job, direct),
        super::attach_step::Picked::Stopped => {}
        super::attach_step::Picked::Upload => {
            if let Some(job) = attach_file_job(state, path) {
                super::attach_step::open(state, job, path);
            }
        }
    }
}

/// How the chat names the open conversation in "Drop to send in ...": a DM by the person's name,
/// a group by its name, a room as `#room`.
pub(crate) fn conversation_name(state: &GuiState) -> String {
    let ch = &state.chat_active_channel;
    if let Some(pk) = ch.strip_prefix("dm:") {
        state.chat_dms.iter().find(|d| d.user_key == pk).map(|d| d.user_name.clone()).unwrap_or_else(|| "this conversation".to_string())
    } else if let Some(gid) = ch.strip_prefix("p2pgroup:") {
        state.p2p_groups.iter().find(|g| g.group_id == gid).map(|g| g.name.clone()).unwrap_or_else(|| "this group".to_string())
    } else {
        format!("#{}", super::commons_room_of(ch).unwrap_or(ch))
    }
}

/// The words over the window while files are dragged onto the chat.
pub(crate) fn drop_words(state: &GuiState) -> String {
    format!("Drop to send in {}", conversation_name(state))
}

/// Files dropped on the window over the chat (gui/drop_files.rs). Over one of this server's
/// channels: the upload window (Add to Media, every control) with "Also post in chat" ticked for
/// that channel. Over a private conversation (a direct message, a group, the scratch pad, or a
/// room this server does not carry): sent as that conversation's Attach sends a file, each
/// through its attach step, encrypted where the conversation is, and never made public. With no
/// conversation open, the upload window (the app's own default) takes them.
pub(super) fn offer_drop(ctx: &egui::Context, state: &GuiState) {
    use crate::gui::drop_files::{self, Level};
    let chosen_in = state.chat_active_channel.clone();
    if chosen_in.is_empty() {
        return;
    }
    if let Some(channel) = state.chat_channels.iter().find(|c| c.id == chosen_in) {
        // A read-only channel (announcements) takes posts from admins only: for anyone else the
        // upload window opens with nothing ticked, and says so before the drop (the website's rule).
        if channel.post_level.staff_only() && !crate::gui::pages::files::is_admin(state) {
            drop_files::register(ctx, Level::Page, crate::gui::pages::files::DROP_TO_UPLOAD, |state: &mut GuiState, paths| {
                crate::gui::pages::files::upload_dropped(state, &paths, None);
            });
            return;
        }
        let words = crate::gui::pages::files::drop_and_post_words(&conversation_name(state));
        drop_files::register(ctx, Level::Page, words, move |state: &mut GuiState, paths| {
            crate::gui::pages::files::upload_dropped(state, &paths, Some(chosen_in));
        });
        return;
    }
    drop_files::register(ctx, Level::Page, drop_words(state), move |state: &mut GuiState, paths| {
        // Only into the conversation that was open when they were let go.
        if state.chat_active_channel == chosen_in {
            start_attach_all(state, paths);
        }
    });
}

/// Files chosen together (operator, 2026-10-16: several files at once): the first starts now,
/// the rest wait in `FilesFlow::queue` for the conversation they were chosen in.
pub(super) fn start_attach_all(state: &mut GuiState, paths: Vec<std::path::PathBuf>) {
    let mut paths = std::collections::VecDeque::from(paths);
    let Some(first) = paths.pop_front() else { return };
    if !paths.is_empty() {
        state.files_flow.queue = Some((state.chat_active_channel.clone(), paths));
    }
    start_attach(state, &first);
}

/// The next of several files chosen together, once the one before is done: its step closed (sent
/// or cancelled) and its upload finished. Each goes as a single file goes, one message each. They
/// belong to the conversation they were chosen in: a file chosen in a DM must never go anywhere
/// else, so leaving it drops the rest and says so. Every frame (chat.rs `draw`).
pub(super) fn next_queued(state: &mut GuiState) {
    let Some((channel, waiting)) = state.files_flow.queue.as_ref() else { return };
    if *channel != state.chat_active_channel {
        let n = waiting.len();
        state.files_flow.queue = None;
        state.pending_notices.push(left_before_the_rest(n));
        return;
    }
    if state.files_flow.step.is_some() || state.clipboard_upload.is_some() {
        return;
    }
    let next = state.files_flow.queue.as_mut().and_then(|(_, q)| q.pop_front());
    if state.files_flow.queue.as_ref().is_some_and(|(_, q)| q.is_empty()) {
        state.files_flow.queue = None;
    }
    if let Some(path) = next {
        start_attach(state, &path);
    }
}

/// What is said when the person leaves a conversation while files chosen for it are still to go.
pub(crate) fn left_before_the_rest(n: usize) -> String {
    if n == 1 {
        return "The last file you chose was not sent, because you left the conversation it was for.".to_string();
    }
    format!("The last {n} files you chose were not sent, because you left the conversation they were for.")
}

/// An image was pasted (Ctrl+V with a picture on the clipboard): start sending it, unless sending
/// files is off in private conversations and this is one (Settings > Privacy, Files in messages).
pub(super) fn start_paste(state: &mut GuiState, png: Vec<u8>) {
    if super::attach_step::attach_off_here(state) {
        state.pending_notices.push(crate::net::files_policy::ATTACH_OFF.to_string());
        return;
    }
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
            state.files_flow.sent_public = None;
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
            let sent = state.files_flow.sent_public.take();
            // The server could not remove its hidden details (BUG-260): the person is asked ("Don't
            // send" first), in the conversation it was for, instead of being told it failed. Never
            // under the protected setup, whose promise is that a file goes only with its location
            // removed (2026-11-07, BUG-276): the server's words then, as for any other refusal.
            let protected = crate::engine::protected::is_on(state);
            if let (Some(job), true, false) = (sent, e.starts_with(CANNOT_CLEAN_TAG), protected) {
                log::info!("Attachment upload: the server could not clean it; asking the person");
                state.files_flow.server_ask = Some((made_for, job));
                return;
            }
            let e = e.trim_start_matches(CANNOT_CLEAN_TAG).to_string();
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

/// Blocking multipart upload of any file to `<server_url>/api/upload?token=<this connection's token>`,
/// `share` adding `&share=1` (a 3D or model file for the public Shared Files library) and
/// `encrypted` adding `&encrypted=1` (opaque ciphertext: the server skips format and EXIF handling).
/// Returns the URL from the JSON response. The app's own sends go through `upload_flagged`; this
/// form is the tests'.
#[cfg(test)]
pub(crate) fn upload_file_blocking_ext(
    server_url: &str,
    token: &str,
    filename: &str,
    mime: &str,
    bytes: Vec<u8>,
    share: bool,
    encrypted: bool,
) -> Result<String, String> {
    upload_flagged(server_url, token, filename, mime, bytes, UploadFlags { share, encrypted, keep_metadata: false })
}

/// Which shelf an upload goes on, as `/api/upload`'s query names it (relay/files.rs
/// `upload_shelf`): `share` the public Shared Files list, `encrypted` an opaque private file.
/// Neither: a chat file. (A Media file goes in pieces instead: files/resumable.rs.)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct UploadFlags {
    pub share: bool,
    pub encrypted: bool,
    /// The person chose to keep the file's hidden details (B4): the relay then skips its own strip.
    pub keep_metadata: bool,
}

/// The query of one upload: this connection's token, then each flag that is set.
pub(crate) fn upload_query(token: &str, flags: UploadFlags) -> String {
    let mut q = format!("token={token}");
    if flags.share {
        q.push_str("&share=1");
    }
    if flags.encrypted {
        q.push_str("&encrypted=1");
    }
    if flags.keep_metadata {
        q.push_str("&keep_metadata=1");
    }
    q
}

/// [`upload_with_poster_keep`] with no poster picture and no keep choice: the tests' form.
#[cfg(test)]
pub(crate) fn upload_flagged(server_url: &str, token: &str, filename: &str, mime: &str, bytes: Vec<u8>, flags: UploadFlags) -> Result<String, String> {
    upload_with_poster_keep(server_url, token, filename, mime, bytes, flags, None, None).map_err(|(_, words)| words)
}

/// The one multipart upload every kind of file goes through, with `flags` in its query, a video's
/// poster picture as the form's `poster` part before the file's (2026-11-06, relay/poster.rs), and
/// the person's signed choice to keep the file's hidden details (`keep`, its JSON,
/// `KeepSigner::choice`) as the `keep` part before everything (BUG-260), which the relay requires
/// whenever `flags.keep_metadata` is set. The body is streamed around the file's own buffer
/// (attach_body.rs `Multipart`), its length set so it is sent as it is: the file is never copied
/// into a second buffer with its framing. Err carries the server's status when it refused (422:
/// the file's hidden details could not be removed) and the words.
#[allow(clippy::too_many_arguments)]
pub(crate) fn upload_with_poster_keep(server_url: &str, token: &str, filename: &str, mime: &str, bytes: Vec<u8>, flags: UploadFlags, poster: Option<Vec<u8>>, keep: Option<&str>) -> Result<String, (Option<u16>, String)> {
    let base = server_url.trim_end_matches('/');
    let upload_url = format!("{base}/api/upload?{}", upload_query(token, flags));
    let mut body = match poster {
        Some(p) => attach_body::Multipart::with_poster(filename, mime, bytes, &p),
        None => attach_body::Multipart::new(filename, mime, bytes),
    };
    if let Some(k) = keep {
        body = body.with_text_part_first(crate::relay::upload_rules::KEEP_FIELD, k);
    }
    let resp = ureq::post(&upload_url)
        .set("Content-Type", &format!("multipart/form-data; boundary={}", body.boundary))
        .set("Content-Length", &body.len().to_string())
        .send(body.reader())
        .map_err(|e| {
            let status = if let ureq::Error::Status(code, _) = &e { Some(*code) } else { None };
            (status, upload_error_words(e))
        })?;
    let body_str = resp.into_string().map_err(|e| (None, format!("read response: {e}")))?;
    let val: serde_json::Value = serde_json::from_str(&body_str).map_err(|e| (None, format!("parse JSON: {e}; body={body_str}")))?;
    val.get("url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| (None, format!("response missing 'url' field: {body_str}")))
}

/// What a failed upload says to the person: the server's own words when it refused (its body:
/// "File too large ... your limit on this server is N MB", "Your role isn't allowed ..."), or that
/// the server could not be reached. Never the upload token: ureq's error text holds the whole
/// address, `?token=` and all, and these words reach the screen and run.log (`hide_token`).
pub(crate) fn upload_error_words(e: ureq::Error) -> String {
    let words = match e {
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
    };
    crate::gui::widgets::image_cache::hide_token(&words)
}

#[cfg(test)]
#[path = "attach_send_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "attach_send_clean_tests.rs"]
mod clean_tests;
// A file that could not be cleaned: asked about, and sent only with the signed choice (BUG-260).
#[cfg(test)]
#[path = "attach_keep_tests.rs"]
mod keep_tests;
