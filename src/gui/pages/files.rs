//! File Browser page — browse data/ directory, view and edit text files.

use egui::{Color32, Frame, RichText, ScrollArea, Vec2};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;
use std::path::{Path, PathBuf};

/// A node in the directory tree. Expansion + selection state lives in
/// `FileBrowserState::tree` (a `widgets::TreeState`), keyed by absolute path.
#[derive(Debug, Clone)]
pub enum FsNode {
    Dir {
        name: String,
        path: PathBuf,
        children: Vec<FsNode>,
    },
    File {
        name: String,
        path: PathBuf,
    },
}

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

/// Local state for the file browser.
pub struct FileBrowserState {
    pub root: Option<FsNode>,
    pub selected_file: Option<PathBuf>,
    pub file_content: String,
    pub file_dirty: bool,
    pub breadcrumbs: Vec<String>,
    pub status_message: String,
    pub initialized: bool,
    /// Universal tree widget state — owns expand/collapse + selection.
    pub tree: widgets::TreeState,

    // ── Shared-files manager (v0.710): add/remove files on the SERVER ──
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
}

impl Default for FileBrowserState {
    fn default() -> Self {
        Self {
            root: None,
            selected_file: None,
            file_content: String::new(),
            file_dirty: false,
            breadcrumbs: vec!["data".into()],
            status_message: String::new(),
            initialized: false,
            tree: widgets::TreeState::new(),
            shared_files: Vec::new(),
            shared_status: String::new(),
            shared_loaded: false,
            shared_list_rx: None,
            shared_action_rx: None,
            shared_picker: None,
        }
    }
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

fn scan_dir(path: &Path, depth: usize) -> Option<FsNode> {
    if depth > 4 {
        return None;
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "data".into());

    if path.is_dir() {
        let mut children = Vec::new();
        if let Ok(entries) = std::fs::read_dir(path) {
            let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
            entries.sort_by_key(|e| {
                let is_file = e.file_type().map(|ft| ft.is_file()).unwrap_or(false);
                (is_file, e.file_name())
            });
            for entry in entries {
                let ep = entry.path();
                if let Some(node) = scan_dir(&ep, depth + 1) {
                    children.push(node);
                }
            }
        }
        Some(FsNode::Dir {
            name,
            path: path.to_path_buf(),
            children,
        })
    } else if path.is_file() {
        Some(FsNode::File {
            name,
            path: path.to_path_buf(),
        })
    } else {
        None
    }
}

/// Render the directory tree using the universal `widgets::tree_node` /
/// `widgets::tree_leaf` primitives. Expansion + selection state lives in
/// `tree`; this function only owns the file-load side effect.
fn draw_tree(
    ui: &mut egui::Ui,
    node: &FsNode,
    theme: &Theme,
    tree: &mut widgets::TreeState,
    selected: &mut Option<PathBuf>,
    content: &mut String,
    dirty: &mut bool,
    breadcrumbs: &mut Vec<String>,
    status: &mut String,
) {
    match node {
        FsNode::Dir { name, path, children } => {
            let id = path.to_string_lossy().to_string();
            widgets::tree_node(ui, theme, tree, &id, name, None, |ui, tree| {
                for child in children {
                    draw_tree(ui, child, theme, tree, selected, content, dirty, breadcrumbs, status);
                }
            });
        }
        FsNode::File { name, path } => {
            let id = path.to_string_lossy().to_string();
            // Sync external "selected_file" state with the tree's notion of selection.
            if selected.as_ref() == Some(path) && !tree.is_selected(&id) {
                tree.select(&id);
            }
            let resp = widgets::tree_leaf(ui, theme, tree, &id, name, None);
            if resp.clicked {
                match std::fs::read_to_string(path) {
                    Ok(text) => {
                        *content = text;
                        *dirty = false;
                        *status = String::new();
                    }
                    Err(e) => {
                        *content = format!("(Could not read file: {})", e);
                        *dirty = false;
                        *status = format!("Error: {}", e);
                    }
                }
                *selected = Some(path.clone());
                *breadcrumbs = path
                    .components()
                    .filter_map(|c| {
                        let s = c.as_os_str().to_string_lossy().to_string();
                        if s == "." || s == ".." { None } else { Some(s) }
                    })
                    .collect();
            }
        }
    }
}

/// Thread-local state for the file browser.
fn with_state<R>(f: impl FnOnce(&mut FileBrowserState) -> R) -> R {
    use std::cell::RefCell;
    thread_local! {
        static STATE: RefCell<FileBrowserState> = RefCell::new(FileBrowserState::default());
    }
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// The "Shared files on the server" section: list, upload (via the in-app file
/// browser), and remove. The operator's need to add AND remove files people can
/// download, from the native app (v0.710).
fn draw_shared_files_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let server = state.server_url.clone();
    let my_key = state.profile_public_key.clone();
    let seed = state.private_key_bytes.clone();
    // Admin = server-known role for my key (from the chat user list). Admins can
    // remove any file; everyone can remove their own. The server enforces this
    // regardless of what the button shows.
    let is_admin = state
        .chat_users
        .iter()
        .find(|u| u.public_key == my_key)
        .map(|u| u.role == "admin")
        .unwrap_or(false);

    // Drain in-flight results + auto-load once.
    let mut trigger_refresh = false;
    with_state(|fs| {
        if !fs.shared_loaded && fs.shared_list_rx.is_none() && !server.is_empty() {
            trigger_refresh = true;
        }
        if let Some(rx) = fs.shared_list_rx.as_ref() {
            if let Ok(res) = rx.try_recv() {
                fs.shared_list_rx = None;
                fs.shared_loaded = true;
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
    });

    let start_refresh = |server: String| {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(fetch_shared_blocking(&server));
        });
        rx
    };
    if trigger_refresh && !server.is_empty() {
        let rx = start_refresh(server.clone());
        with_state(|fs| {
            fs.shared_list_rx = Some(rx);
            fs.shared_status = "Loading...".to_string();
        });
    }

    ui.label(
        RichText::new("Shared files on the server")
            .size(theme.font_size_body)
            .strong()
            .color(theme.text_primary()),
    );
    ui.label(
        RichText::new("Files here are public for anyone to download. Upload to add one; remove to take it down.")
            .size(theme.font_size_small)
            .color(theme.text_muted()),
    );
    ui.add_space(theme.spacing_xs);

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
    ui.horizontal(|ui| {
        if widgets::Button::primary("Upload a file").show(ui, theme) {
            open_picker = true;
        }
        if widgets::Button::secondary("Refresh").show(ui, theme) {
            do_refresh = true;
        }
        let status = with_state(|fs| fs.shared_status.clone());
        if !status.is_empty() {
            ui.label(RichText::new(status).size(theme.font_size_small).color(theme.text_muted()));
        }
    });
    if do_refresh {
        let rx = start_refresh(server.clone());
        with_state(|fs| fs.shared_list_rx = Some(rx));
    }
    if open_picker {
        with_state(|fs| {
            fs.shared_picker = Some(widgets::file_browser::FilePickerState::new(
                crate::gui::pages::chat::ATTACH_EXTS,
                crate::gui::pages::chat::ATTACH_MAX_BYTES,
            ));
        });
    }

    // The file list.
    let rows = with_state(|fs| fs.shared_files.clone());
    let mut remove_filename: Option<String> = None;
    egui::ScrollArea::vertical()
        .id_salt("shared_files_list")
        .max_height(220.0)
        .show(ui, |ui| {
            if rows.is_empty() {
                ui.label(
                    RichText::new("No shared files yet.")
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            }
            for row in &rows {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&row.name)
                            .size(theme.font_size_small)
                            .color(theme.text_primary()),
                    );
                    ui.label(
                        RichText::new(format!(
                            "({}) by {}",
                            crate::gui::widgets::file_browser::human_size(row.size_bytes.max(0) as u64),
                            if row.uploader_name.is_empty() { "unknown" } else { &row.uploader_name }
                        ))
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                    );
                    let can_remove = is_admin || row.uploader_key == my_key;
                    if can_remove && widgets::Button::danger("Remove").show(ui, theme) {
                        remove_filename = Some(filename_from_url(&row.url).to_string());
                    }
                });
            }
        });

    if let Some(filename) = remove_filename {
        if let Some(seed) = seed.clone() {
            let (tx, rx) = std::sync::mpsc::channel();
            let server = server.clone();
            let key = my_key.clone();
            std::thread::spawn(move || {
                let _ = tx.send(delete_shared_blocking(&server, &filename, &key, &seed));
            });
            with_state(|fs| {
                fs.shared_action_rx = Some(rx);
                fs.shared_status = "Removing...".to_string();
            });
        } else {
            with_state(|fs| fs.shared_status = "Unlock your identity first to remove files.".to_string());
        }
    }

    // Upload picker modal.
    let picker_open = with_state(|fs| fs.shared_picker.is_some());
    if picker_open {
        use crate::gui::widgets::file_browser::{file_picker_modal, FilePickerResult};
        let mut picker = with_state(|fs| fs.shared_picker.take().unwrap());
        match file_picker_modal(ui.ctx(), theme, &mut picker, "Upload a file to the server") {
            FilePickerResult::Open => {
                with_state(|fs| fs.shared_picker = Some(picker));
            }
            FilePickerResult::Cancelled => {}
            FilePickerResult::Picked(path) => {
                let filename = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file")
                    .to_string();
                match std::fs::read(&path) {
                    Ok(bytes) if (bytes.len() as u64) <= crate::gui::pages::chat::ATTACH_MAX_BYTES => {
                        let mime = "application/octet-stream".to_string();
                        let server = server.clone();
                        let key = my_key.clone();
                        let (tx, rx) = std::sync::mpsc::channel();
                        std::thread::spawn(move || {
                            // share=true: this file joins the public library.
                            let res = crate::gui::pages::chat::upload_file_blocking(
                                &server, &key, &filename, &mime, bytes, true,
                            )
                            .map(|url| format!("uploaded {url}"));
                            let _ = tx.send(res);
                        });
                        with_state(|fs| {
                            fs.shared_action_rx = Some(rx);
                            fs.shared_status = "Uploading...".to_string();
                        });
                    }
                    Ok(_) => {
                        with_state(|fs| fs.shared_status =
                            "File is too large (6 MB max).".to_string());
                    }
                    Err(e) => {
                        with_state(|fs| fs.shared_status = format!("Read failed: {e}"));
                    }
                }
            }
        }
    }
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Initialize on first draw
    with_state(|fs| {
        if !fs.initialized {
            let data_path = PathBuf::from("data");
            if data_path.is_dir() {
                fs.root = scan_dir(&data_path, 0);
                // Seed the universal tree state so the root directory starts open.
                if let Some(FsNode::Dir { path, .. }) = &fs.root {
                    fs.tree.expand(&path.to_string_lossy());
                }
            }
            fs.initialized = true;
        }
    });

    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(16.0))
        .show(ctx, |ui| {
            ui.label(RichText::new("File Browser").size(theme.font_size_title).color(theme.text_primary()));
            ui.add_space(theme.spacing_xs);

            // ── Shared files ON THE SERVER (v0.710): add + remove files that
            // everyone can download. Uses the in-app file browser for uploads
            // and the signed remove endpoint. ──
            draw_shared_files_section(ui, theme, state);
            ui.separator();
            ui.add_space(theme.spacing_sm);
            ui.label(
                RichText::new("Local data files")
                    .size(theme.font_size_body)
                    .strong()
                    .color(theme.text_secondary()),
            );
            ui.add_space(theme.spacing_xs);

            // Breadcrumb navigation
            with_state(|fs| {
                ui.horizontal(|ui| {
                    for (i, crumb) in fs.breadcrumbs.iter().enumerate() {
                        if i > 0 {
                            ui.label(RichText::new("/").color(theme.text_muted()));
                        }
                        ui.label(RichText::new(crumb).color(theme.text_secondary()));
                    }
                });
            });
            ui.separator();

            // Two-panel layout
            ui.columns(2, |cols| {
                // Left panel: directory tree
                cols[0].label(
                    RichText::new("Files")
                        .size(theme.font_size_body)
                        .color(theme.text_secondary()),
                );
                ScrollArea::vertical()
                    .id_salt("file_tree")
                    .show(&mut cols[0], |ui| {
                        with_state(|fs| {
                            // Split-borrow so we can read from `root` while
                            // writing to `tree` and the file-load fields.
                            let FileBrowserState {
                                root,
                                tree,
                                selected_file,
                                file_content,
                                file_dirty,
                                breadcrumbs,
                                status_message,
                                ..
                            } = &mut *fs;
                            if let Some(root) = root.as_ref() {
                                draw_tree(
                                    ui,
                                    root,
                                    theme,
                                    tree,
                                    selected_file,
                                    file_content,
                                    file_dirty,
                                    breadcrumbs,
                                    status_message,
                                );
                            } else {
                                ui.label(
                                    RichText::new("data/ directory not found")
                                        .color(theme.text_muted()),
                                );
                            }
                        });
                    });

                // Right panel: file content viewer
                with_state(|fs| {
                    if fs.selected_file.is_some() {
                        ScrollArea::vertical()
                            .id_salt("file_content")
                            .show(&mut cols[1], |ui| {
                                let resp = ui.add(
                                    egui::TextEdit::multiline(&mut fs.file_content)
                                        .desired_width(f32::INFINITY)
                                        .font(egui::TextStyle::Monospace),
                                );
                                if resp.changed() {
                                    fs.file_dirty = true;
                                }
                            });

                        // Save button
                        cols[1].horizontal(|ui| {
                            let enabled = fs.file_dirty;
                            ui.add_enabled_ui(enabled, |ui| {
                                if widgets::primary_button(ui, theme, "Save") {
                                    if let Some(ref path) = fs.selected_file {
                                        match std::fs::write(path, &fs.file_content) {
                                            Ok(_) => {
                                                fs.file_dirty = false;
                                                fs.status_message = "Saved.".into();
                                            }
                                            Err(e) => {
                                                fs.status_message = format!("Save failed: {}", e);
                                            }
                                        }
                                    }
                                }
                            });
                            if !fs.status_message.is_empty() {
                                ui.label(
                                    RichText::new(&fs.status_message)
                                        .color(theme.text_muted())
                                        .size(theme.font_size_small),
                                );
                            }
                        });
                    } else {
                        cols[1].centered_and_justified(|ui| {
                            ui.label(
                                RichText::new("Select a file to view")
                                    .color(theme.text_muted()),
                            );
                        });
                    }
                });
            });
        });
}
