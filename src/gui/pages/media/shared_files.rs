//! The Files page's "Shared files" place (2026-10-19): the server's public file library, the files
//! anyone can download (GET /api/uploads), with Upload and Remove. It was the top half of the
//! developer's Files page (v0.710: the operator's need to add AND remove files people can download,
//! from the native app) until the two pages became one Files page (operator, 2026-10-19: "What's
//! the real difference between these two? Why would we not want to merge them?").
//!
//! In the three panes: on the left, under the places, the list (Upload a file, Refresh, what was
//! last done, and a row per file: its type's badge, its name, its size and who shared it); in the
//! middle the file chosen, as the chat shows a public file (chat/public_files.rs: Download, View in
//! 3D for a model, Play here for a video the app plays); on the right what it is, with Remove (your
//! own, or any for an admin; the server checks either way) and Put on the Files page (your own:
//! the Shared on <server> list's form opens for it, public only).
//!
//! Files dropped on the window while this place is shown go to Shared Files, as files chosen with
//! Upload a file go (gui/drop_files.rs). Each picked file has its look first (what is hidden in it,
//! and the switch that removes it: shared_upload_step.rs), and goes up on a worker, never on the
//! frame.

use egui::RichText;

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

/// The place's name in the places list.
pub const PLACE: &str = "Shared files";
/// The place's heading over its list, and its promise.
pub const HEADING: &str = "Shared files on the server";
pub const HINT: &str = "Files here are public for anyone to download. Upload to add one; remove to take it down.";
/// The middle and right panes before a file is chosen.
pub const CHOOSE_LINE: &str = "Choose a file on the left to see it here.";

/// One file in the server's shared library (v0.710). Mirrors the JSON
/// GET /api/uploads returns.
#[derive(Debug, Clone)]
pub struct SharedFileRow {
    pub url: String,
    pub name: String,
    pub size_bytes: i64,
    pub uploader_key: String,
    pub uploader_name: String,
}

/// The stored filename (basename) is the last segment of the `/uploads/...`
/// URL — what POST /api/uploads/delete expects.
fn filename_from_url(url: &str) -> &str {
    url.rsplit('/').next().unwrap_or(url)
}

type ListResult = Result<Vec<SharedFileRow>, String>;
type ActionResult = Result<String, String>;

/// The place's state, kept in `MediaState::shared` (and forgotten with the rest of the page's state
/// when the server or the person changes).
#[derive(Default)]
pub struct SharedFiles {
    /// The server (normalised address) the shared list below is for: on another, it is forgotten
    /// and asked for again ([`forget_other_server`]), so a row of one server's list can never be
    /// put on another's Files page.
    pub shared_server: String,
    /// The server's public shared-file library (GET /api/uploads).
    pub shared_files: Vec<SharedFileRow>,
    /// One-line feedback for the shared section (uploading/removing/errors).
    pub shared_status: String,
    /// Load the library once on first view.
    pub shared_loaded: bool,
    /// In-flight list fetch.
    pub shared_list_rx: Option<std::sync::mpsc::Receiver<ListResult>>,
    /// In-flight upload/remove; on success we refresh the list.
    pub shared_action_rx: Option<std::sync::mpsc::Receiver<ActionResult>>,
    /// Open file picker for uploading to the server.
    pub shared_picker: Option<widgets::file_browser::FilePickerState>,
    /// A picked file being looked at before it uploads (shared_upload_step.rs, 2026-10-11).
    pub shared_step: Option<super::shared_upload_step::UploadStep>,
    /// Files chosen together still to go up (2026-10-16), one at a time through the same step:
    /// the next once the one before is uploaded or cancelled.
    pub shared_queue: std::collections::VecDeque<std::path::PathBuf>,
    /// The file chosen in the list (its address), shown in the middle and on the right.
    pub selected: Option<String>,
}

/// The shared list belongs to one server: when the app is on another (`here`, normalised), the
/// list, its status and any answer still on its way are forgotten, and the list is asked for
/// again. Before 2026-10-11 the old server's rows stayed, and Put on Media on one of them named a
/// file the new server does not have.
pub(crate) fn forget_other_server(fs: &mut SharedFiles, here: &str) {
    if fs.shared_server == here {
        return;
    }
    fs.shared_server = here.to_string();
    fs.shared_files.clear();
    fs.shared_status.clear();
    fs.shared_loaded = false;
    fs.shared_list_rx = None;
    fs.shared_action_rx = None;
    fs.selected = None;
}

/// Blocking GET /api/uploads -> the shared-file library. Runs on a worker
/// thread at the call site.
fn fetch_shared_blocking(server_url: &str) -> ListResult {
    let base = server_url.trim_end_matches('/');
    let url = format!("{base}/api/uploads?limit=500");
    let resp = ureq::get(&url)
        .call()
        .map_err(|e| format!("list failed: {e}"))?;
    let body = resp.into_string().map_err(|e| format!("read: {e}"))?;
    let val: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("parse: {e}"))?;
    // The handler returns { files: [...] } or a bare array; accept both.
    let arr = val
        .get("files")
        .and_then(|f| f.as_array())
        .or_else(|| val.as_array())
        .cloned()
        .unwrap_or_default();
    let rows = arr
        .into_iter()
        .map(|f| SharedFileRow {
            url: f.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            name: f.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            size_bytes: f.get("size_bytes").and_then(|v| v.as_i64()).unwrap_or(0),
            uploader_key: f.get("uploader_key").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            uploader_name: f.get("uploader_name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        })
        .collect();
    Ok(rows)
}

/// Blocking POST /api/uploads/delete (signed). Removes a shared file the
/// caller owns, or any file when the caller is an admin (server enforces).
fn delete_shared_blocking(
    server_url: &str,
    filename: &str,
    public_key: &str,
    seed: &[u8],
) -> ActionResult {
    let base = server_url.trim_end_matches('/');
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let sig = crate::net::identity::pq_sign_chat(seed, "delete_upload", ts);
    let body = serde_json::json!({
        "filename": filename,
        "key": public_key,
        "timestamp": ts,
        "sig": sig,
    });
    let resp = ureq::post(&format!("{base}/api/uploads/delete"))
        .set("Content-Type", "application/json")
        .send_string(&body.to_string());
    match resp {
        Ok(_) => Ok(filename.to_string()),
        Err(ureq::Error::Status(code, r)) => {
            let msg = r.into_string().unwrap_or_default();
            Err(format!("remove failed ({code}): {msg}"))
        }
        Err(e) => Err(format!("remove failed: {e}")),
    }
}

/// Whether this person is an admin of the server, by the role the chat's member list names for
/// their key. Admins can remove any file; everyone can remove their own. The server enforces this
/// regardless of what the button shows.
fn admin_here(state: &GuiState) -> bool {
    let me = &state.profile_public_key;
    state.chat_users.iter().find(|u| &u.public_key == me).is_some_and(|u| u.role == "admin")
}

fn start_refresh(server: String) -> std::sync::mpsc::Receiver<ListResult> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(fetch_shared_blocking(&server));
    });
    rx
}

/// Every frame the place is shown: forget another server's list, read what came back, and ask
/// for the list once.
fn poll(state: &mut GuiState) {
    let server = state.server_url.clone();
    let here = crate::gui::pages::chat::norm_server_url(&server);
    let fs = &mut state.media.shared;
    forget_other_server(fs, &here);
    let mut trigger_refresh = !fs.shared_loaded && fs.shared_list_rx.is_none() && !server.is_empty();
    if let Some(rx) = fs.shared_list_rx.as_ref() {
        if let Ok(res) = rx.try_recv() {
            fs.shared_list_rx = None;
            fs.shared_loaded = true;
            trigger_refresh = false;
            match res {
                Ok(rows) => {
                    fs.shared_status = format!("{} file(s) on the server", rows.len());
                    fs.shared_files = rows;
                }
                Err(e) => fs.shared_status = e,
            }
        }
    }
    if let Some(rx) = fs.shared_action_rx.as_ref() {
        if let Ok(res) = rx.try_recv() {
            fs.shared_action_rx = None;
            match res {
                Ok(done) => {
                    fs.shared_status = format!("Done: {done}");
                    trigger_refresh = true; // reflect the change
                }
                Err(e) => fs.shared_status = e,
            }
        }
    }
    if trigger_refresh && !server.is_empty() {
        fs.shared_list_rx = Some(start_refresh(server));
        fs.shared_status = "Loading...".to_string();
    }
}

/// The left pane's part, under the places: the heading, Upload a file and Refresh, what was last
/// done, a row per file (a press on one shows it in the middle and on the right), the upload's
/// step while a picked file is being looked at, and the picker.
pub(crate) fn draw_list(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    poll(state);
    let server = state.server_url.clone();
    // This connection's upload token: every upload carries it (2026-10-11).
    let upload_token = crate::gui::pages::chat::upload_token_here(state);
    // My own upload limit on this server (its peer list says; 2026-10-10), the chat's fallback until then.
    let upload_limit = crate::gui::pages::chat::room_limit_here(state).unwrap_or(crate::gui::pages::chat::ATTACH_MAX_BYTES);
    // Files dropped on the window go to Shared Files, as files chosen with its button go
    // (gui/drop_files.rs).
    if server.is_empty() {
        crate::gui::drop_files::refuse(ui.ctx(), crate::gui::drop_files::Level::Page, "Files cannot be dropped here: connect to a server first, on the Chat page.");
    } else {
        let signed_in = upload_token.is_some();
        crate::gui::drop_files::register(ui.ctx(), crate::gui::drop_files::Level::Page, DROP_TO_SHARE, move |state: &mut GuiState, paths| shared_picked(state, paths, signed_in, upload_limit));
    }

    widgets::subsection_label(ui, theme, HEADING);
    widgets::body_hint(ui, theme, HINT);
    if server.is_empty() {
        ui.label(
            RichText::new("Connect to a server first (Chat page) to manage its files.")
                .size(theme.font_size_small)
                .color(theme.warning()),
        );
        return;
    }

    let mut open_picker = false;
    let mut do_refresh = false;
    ui.horizontal_wrapped(|ui| {
        if widgets::Button::primary("Upload a file").show(ui, theme) {
            open_picker = true;
        }
        if widgets::Button::secondary("Refresh").show(ui, theme) {
            do_refresh = true;
        }
    });
    let status = state.media.shared.shared_status.clone();
    if !status.is_empty() {
        ui.label(RichText::new(status).size(theme.font_size_small).color(theme.text_muted()));
    }
    if do_refresh {
        state.media.shared.shared_list_rx = Some(start_refresh(server.clone()));
    }
    if open_picker {
        state.media.shared.shared_picker = Some(
            widgets::file_browser::FilePickerState::new(crate::gui::pages::chat::ATTACH_EXTS, upload_limit)
                .with_pick_verb("Upload")
                .multiple(),
        );
    }

    // The picked file's step: what is hidden in it and the switch, when it has something to say;
    // the upload starts on Upload, or at once for a file with nothing hidden in it.
    if let Some(mut step) = state.media.shared.shared_step.take() {
        use super::shared_upload_step::Outcome;
        let token = upload_token.clone().unwrap_or_default();
        match super::shared_upload_step::draw(ui, theme, state, &mut step, &server, &token) {
            Outcome::Waiting => state.media.shared.shared_step = Some(step),
            Outcome::Uploading(rx) => {
                state.media.shared.shared_action_rx = Some(rx);
                state.media.shared.shared_status = "Uploading...".to_string();
            }
            Outcome::Cancelled => {}
        }
    }

    // The file list: a row per file, the one chosen highlighted.
    let rows = state.media.shared.shared_files.clone();
    let selected = state.media.shared.selected.clone();
    let mut picked: Option<String> = None;
    egui::ScrollArea::vertical().id_salt("shared_files_list").auto_shrink([false, false]).show(ui, |ui| {
        if rows.is_empty() {
            ui.label(
                RichText::new("No shared files yet.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
        }
        for row in &rows {
            ui.horizontal(|ui| {
                // Its type first, always (widgets/file_badge.rs, 2026-10-17).
                widgets::file_badge::show(ui, theme, &crate::net::file_risk::file_type(&row.name, None));
                let chosen = selected.as_deref() == Some(row.url.as_str());
                let name = RichText::new(&row.name).size(theme.font_size_small).color(theme.text_primary());
                if ui.selectable_label(chosen, name).on_hover_text("Show it in the middle and what it is on the right.").clicked() {
                    picked = Some(row.url.clone());
                }
                ui.label(
                    RichText::new(format!(
                        "({}) by {}",
                        crate::gui::widgets::file_browser::human_size(row.size_bytes.max(0) as u64),
                        if row.uploader_name.is_empty() { "unknown" } else { &row.uploader_name }
                    ))
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
                );
            });
        }
    });
    if picked.is_some() {
        state.media.shared.selected = picked;
    }

    // Upload picker modal.
    if let Some(mut picker) = state.media.shared.shared_picker.take() {
        use crate::gui::widgets::file_browser::{file_picker_modal, FilePickerResult};
        match file_picker_modal(ui.ctx(), theme, &mut picker, "Upload files to the server: choose one or more") {
            FilePickerResult::Open => state.media.shared.shared_picker = Some(picker),
            FilePickerResult::Cancelled => {}
            FilePickerResult::Picked(paths) => shared_picked(state, paths, upload_token.is_some(), upload_limit),
        }
    }
    // The next of several chosen together, once the one before is up or cancelled.
    let fs = &mut state.media.shared;
    if fs.shared_step.is_none() && fs.shared_action_rx.is_none() {
        if let Some(next) = fs.shared_queue.pop_front() {
            start_shared_upload(state, next, upload_token.is_some(), upload_limit);
        }
    }
}

/// The file chosen, if it is still in the list.
fn chosen(state: &GuiState) -> Option<SharedFileRow> {
    let url = state.media.shared.selected.as_deref()?;
    state.media.shared.shared_files.iter().find(|r| r.url == url).cloned()
}

/// The middle pane: the file chosen, as the chat shows a public file (its card: name, size,
/// Download, View in 3D for a model, Play here for a video the app plays).
pub(crate) fn draw_file(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let Some(row) = chosen(state) else {
        super::viewer::nothing_chosen(ui, theme, CHOOSE_LINE);
        return;
    };
    let server = state.server_url.clone();
    let card = crate::gui::pages::chat::public_files::Card::of(&row.url, &server);
    let bg = theme.bg_card();
    // A third of the way down, as the other places' middle panes show a file.
    ui.add_space((ui.available_height() * 0.3 - theme.spacing_lg).max(0.0));
    let crate::gui::GuiState { image_cache, chat_model_view, chat_video, .. } = state;
    crate::gui::pages::chat::public_files::draw(ui, theme, image_cache, chat_model_view, chat_video, &card, bg);
}

/// The right pane: what the file chosen is, with Remove and Put on the Files page.
pub(crate) fn draw_details(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let Some(row) = chosen(state) else {
        super::viewer::nothing_chosen(ui, theme, HINT);
        return;
    };
    let my_key = state.profile_public_key.clone();
    let mut remove = false;
    let mut put = false;
    egui::ScrollArea::vertical().id_salt("shared_file_details").auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            widgets::file_badge::show(ui, theme, &crate::net::file_risk::file_type(&row.name, None));
            ui.label(RichText::new(&row.name).size(theme.font_size_heading).strong().color(theme.text_primary()));
        });
        let by = if row.uploader_name.is_empty() { "unknown" } else { row.uploader_name.as_str() };
        ui.label(
            RichText::new(format!("{}, shared by {by}", crate::gui::widgets::file_browser::human_size(row.size_bytes.max(0) as u64)))
                .size(theme.font_size_small)
                .color(theme.text_secondary()),
        );
        widgets::body_hint(ui, theme, HINT);
        ui.add_space(theme.spacing_sm);
        ui.horizontal_wrapped(|ui| {
            let can_remove = admin_here(state) || row.uploader_key == my_key;
            if can_remove && widgets::Button::danger("Remove").tooltip("Take it off the server's shared files.").show(ui, theme) {
                remove = true;
            }
            if !my_key.is_empty()
                && row.uploader_key == my_key
                && widgets::Button::secondary("Put on the Files page")
                    .tooltip("Show this file on the server's Files page too, in the Shared on list.")
                    .show(ui, theme)
            {
                put = true;
            }
        });
    });
    if put {
        // A row of your own pressed "Put on the Files page": the form opens for it, public only (it
        // is already public here; docs/design/media-showcase.md, increment 2).
        super::put_on_media(state, filename_from_url(&row.url), &row.name, row.size_bytes.max(0) as u64);
    }
    if remove {
        let filename = filename_from_url(&row.url).to_string();
        let fs = &mut state.media.shared;
        if let Some(seed) = state.private_key_bytes.clone() {
            let (tx, rx) = std::sync::mpsc::channel();
            let server = state.server_url.clone();
            std::thread::spawn(move || {
                let _ = tx.send(delete_shared_blocking(&server, &filename, &my_key, &seed));
            });
            fs.shared_action_rx = Some(rx);
            fs.shared_status = "Removing...".to_string();
            fs.selected = None;
        } else {
            fs.shared_status = "Unlock your identity first to remove files.".to_string();
        }
    }
}

/// What the overlay says over Shared files while files are dragged onto it.
pub const DROP_TO_SHARE: &str = "Drop to upload to Shared Files";

/// Files chosen for Shared Files, in the picker or dropped on the window: sizes first, for all of
/// them, one too large said now by name while the rest go; the first starts now and the others wait
/// their turn.
fn shared_picked(state: &mut GuiState, paths: Vec<std::path::PathBuf>, signed_in: bool, upload_limit: u64) {
    let (paths, too_big): (Vec<_>, Vec<_>) = paths
        .into_iter()
        .partition(|p| std::fs::metadata(p).map(|m| m.len() <= upload_limit).unwrap_or(true));
    if !too_big.is_empty() {
        let names: Vec<String> = too_big.iter().filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string())).collect();
        state.media.shared.shared_status = format!("Not uploaded, over the {} MB limit for you on this server: {}.", upload_limit / (1024 * 1024), names.join(", "));
    }
    let mut paths = std::collections::VecDeque::from(paths);
    if let Some(first) = paths.pop_front() {
        state.media.shared.shared_queue = paths;
        start_shared_upload(state, first, signed_in, upload_limit);
    }
}

/// Start one picked file's way to the server: its size checked now, then its step (what is hidden
/// in it), read, cleaned and uploaded by a worker after its look (shared_upload_step.rs,
/// 2026-10-11), never on the frame.
fn start_shared_upload(state: &mut GuiState, path: std::path::PathBuf, signed_in: bool, upload_limit: u64) {
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string();
    match std::fs::metadata(&path).map(|m| m.len()) {
        Ok(_) if !signed_in => {
            let fs = &mut state.media.shared;
            fs.shared_status = crate::gui::pages::chat::NOT_SIGNED_IN_YET.to_string();
            fs.shared_queue.clear();
        }
        Ok(len) if len <= upload_limit => {
            let step = super::shared_upload_step::start(state, path, filename);
            state.media.shared.shared_step = Some(step);
        }
        Ok(_) => {
            state.media.shared.shared_status = format!("{filename} is too large ({} MB max for you on this server).", upload_limit / (1024 * 1024));
        }
        Err(e) => {
            state.media.shared.shared_status = format!("{filename} could not be read: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ANOTHER SERVER'S SHARED LIST IS FORGOTTEN (the 2026-10-11 review of the Media page): on a
    /// server switch the list, its status and any answer still on its way go, and it is asked for
    /// again; the same server keeps what it has.
    ///
    /// Seen red 2026-10-11 with `forget_other_server` keeping the rows: "the old server's rows go"
    /// (left: 1 / right: 0).
    #[test]
    fn another_servers_shared_list_is_forgotten() {
        let mut fs = SharedFiles::default();
        forget_other_server(&mut fs, "https://a.example");
        fs.shared_files.push(SharedFileRow { url: "/uploads/1_a.glb".into(), name: "a.glb".into(), size_bytes: 1, uploader_key: "me".into(), uploader_name: "Me".into() });
        fs.shared_loaded = true;
        fs.shared_status = "1 file(s) on the server".into();
        fs.selected = Some("/uploads/1_a.glb".into());
        forget_other_server(&mut fs, "https://a.example");
        assert_eq!(fs.shared_files.len(), 1, "the same server keeps its list");
        let (_tx, rx) = std::sync::mpsc::channel();
        fs.shared_list_rx = Some(rx);
        forget_other_server(&mut fs, "https://b.example");
        assert_eq!(fs.shared_files.len(), 0, "the old server's rows go");
        assert!(!fs.shared_loaded && fs.shared_list_rx.is_none() && fs.shared_status.is_empty(), "and it is asked for again, its old answer dropped");
        assert!(fs.selected.is_none(), "and the file chosen with them");
    }
}
