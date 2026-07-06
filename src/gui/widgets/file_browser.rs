//! Universal in-app file browser widget (v0.708).
//!
//! The operator's all-in-one direction (2026-07-06): "embed as many tools as
//! possible into the app, even file browsing, to make modding/dev, file
//! uploads/downloads, and whatever else we can even easier." This widget is
//! the seed: ONE reusable browser that today serves the chat attach picker,
//! and later the Files page, download destinations, and the move-my-files
//! storage tool. Deliberately NOT an OS-native dialog (rfd etc.) -- keeping
//! the surface in-app is the point, and it themes/behaves identically
//! everywhere per the universal-widgets rule.
//!
//! Pure listing/filtering logic is separated from the egui layer and
//! unit-tested below.

use std::path::{Path, PathBuf};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use egui::RichText;

/// One entry in a directory listing.
#[derive(Debug, Clone, PartialEq)]
pub struct FsEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
}

/// List a directory: dirs first, then files, both alphabetical
/// (case-insensitive). Files are filtered by `allowed_exts` when non-empty
/// (lowercase, no leading dot; compound extensions like "tar.gz" match by
/// suffix). Unreadable entries are skipped, never an error.
pub fn list_dir(dir: &Path, allowed_exts: &[&str]) -> Vec<FsEntry> {
    let mut dirs: Vec<FsEntry> = Vec::new();
    let mut files: Vec<FsEntry> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let path = e.path();
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            // Hide dotfiles/hidden entries -- this is a user-facing picker,
            // not a power tool (the Files page can grow a toggle later).
            if name.starts_with('.') {
                continue;
            }
            let meta = match e.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.is_dir() {
                dirs.push(FsEntry { name, path, is_dir: true, size: 0 });
            } else {
                if !allowed_exts.is_empty() && !name_matches_ext(&name, allowed_exts) {
                    continue;
                }
                files.push(FsEntry { name, path, is_dir: false, size: meta.len() });
            }
        }
    }
    dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    dirs.extend(files);
    dirs
}

/// Does `name` end with one of the allowed extensions? Case-insensitive;
/// handles compound extensions ("tar.gz") as plain suffix matches.
pub fn name_matches_ext(name: &str, allowed_exts: &[&str]) -> bool {
    let lower = name.to_lowercase();
    allowed_exts.iter().any(|ext| lower.ends_with(&format!(".{ext}")))
}

/// Human-readable size ("412 KB", "3.1 MB").
pub fn human_size(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{} KB", bytes / 1024)
    } else {
        format!("{} B", bytes)
    }
}

/// Quick-access roots for the current user + install: (label, path).
/// Only existing dirs are returned.
pub fn quick_roots() -> Vec<(String, PathBuf)> {
    let mut roots: Vec<(String, PathBuf)> = Vec::new();
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
        .map(PathBuf::from);
    if let Some(h) = home {
        for (label, sub) in [("Home", ""), ("Downloads", "Downloads"), ("Documents", "Documents"), ("Desktop", "Desktop")] {
            let p = if sub.is_empty() { h.clone() } else { h.join(sub) };
            if p.is_dir() {
                roots.push((label.to_string(), p));
            }
        }
    }
    if let Some(data) = crate::storage::writable_data_dir() {
        if data.is_dir() {
            roots.push(("Game data".to_string(), data));
        }
    }
    let exe = crate::storage::exe_dir();
    if exe.is_dir() {
        roots.push(("App folder".to_string(), exe));
    }
    roots
}

/// Modal picker state. `Some` in the caller's GuiState = the modal is open.
#[derive(Debug, Clone)]
pub struct FilePickerState {
    pub current_dir: PathBuf,
    pub selected: Option<FsEntry>,
    /// Lowercase extensions (no dot) the picker offers; empty = all files.
    pub allowed_exts: Vec<String>,
    /// Max selectable file size in bytes (0 = unlimited). Oversized files
    /// list greyed-out with their size so the limit is visible, not silent.
    pub max_size: u64,
}

impl FilePickerState {
    pub fn new(allowed_exts: &[&str], max_size: u64) -> Self {
        let start = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .ok()
            .map(PathBuf::from)
            .filter(|p| p.is_dir())
            .unwrap_or_else(|| crate::storage::exe_dir());
        Self {
            current_dir: start,
            selected: None,
            allowed_exts: allowed_exts.iter().map(|s| s.to_string()).collect(),
            max_size,
        }
    }
}

/// What the picker modal reported this frame.
#[derive(Debug, Clone, PartialEq)]
pub enum FilePickerResult {
    /// Still open, nothing chosen yet.
    Open,
    /// User cancelled/closed.
    Cancelled,
    /// User confirmed this file.
    Picked(PathBuf),
}

/// Draw the modal picker. The caller owns the state (drop it on
/// Cancelled/Picked to close the modal).
pub fn file_picker_modal(
    ctx: &egui::Context,
    theme: &Theme,
    state: &mut FilePickerState,
    title: &str,
) -> FilePickerResult {
    let mut result = FilePickerResult::Open;
    let exts: Vec<&str> = state.allowed_exts.iter().map(|s| s.as_str()).collect();
    let entries = list_dir(&state.current_dir, &exts);

    egui::Window::new(title)
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_size(egui::Vec2::new(560.0, 460.0))
        .frame(egui::Frame::window(&ctx.style()).fill(theme.bg_card()))
        .show(ctx, |ui| {
            // Quick roots row.
            ui.horizontal_wrapped(|ui| {
                for (label, path) in quick_roots() {
                    if widgets::Button::secondary(&label).show(ui, theme) {
                        state.current_dir = path;
                        state.selected = None;
                    }
                }
            });
            ui.add_space(theme.spacing_xs);

            // Current path + up.
            ui.horizontal(|ui| {
                if widgets::Button::secondary("Up").show(ui, theme) {
                    if let Some(parent) = state.current_dir.parent() {
                        state.current_dir = parent.to_path_buf();
                        state.selected = None;
                    }
                }
                ui.label(
                    RichText::new(state.current_dir.display().to_string())
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            });
            ui.add_space(theme.spacing_xs);

            // Entry list.
            egui::ScrollArea::vertical()
                .id_salt("file_picker_list")
                .max_height(300.0)
                .show(ui, |ui| {
                    if entries.is_empty() {
                        ui.label(
                            RichText::new("Nothing here (or nothing matching the allowed types).")
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                        );
                    }
                    for entry in &entries {
                        let selected = state
                            .selected
                            .as_ref()
                            .map(|s| s.path == entry.path)
                            .unwrap_or(false);
                        let too_big =
                            !entry.is_dir && state.max_size > 0 && entry.size > state.max_size;
                        let label = if entry.is_dir {
                            format!("[dir] {}", entry.name)
                        } else {
                            format!("{}  ({})", entry.name, human_size(entry.size))
                        };
                        let color = if too_big {
                            theme.text_muted()
                        } else if entry.is_dir {
                            theme.accent()
                        } else {
                            theme.text_primary()
                        };
                        let resp = ui.selectable_label(
                            selected,
                            RichText::new(label).size(theme.font_size_small).color(color),
                        );
                        if resp.clicked() {
                            if entry.is_dir {
                                state.current_dir = entry.path.clone();
                                state.selected = None;
                            } else if too_big {
                                // Visible but not selectable; the footer explains.
                            } else {
                                state.selected = Some(entry.clone());
                            }
                        }
                        if resp.double_clicked() && !entry.is_dir && !too_big {
                            result = FilePickerResult::Picked(entry.path.clone());
                        }
                    }
                });

            ui.add_space(theme.spacing_xs);
            if state.max_size > 0 {
                ui.label(
                    RichText::new(format!("Max file size: {}", human_size(state.max_size)))
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            }
            ui.horizontal(|ui| {
                let pick_label = state
                    .selected
                    .as_ref()
                    .map(|s| format!("Attach {}", s.name))
                    .unwrap_or_else(|| "Attach".to_string());
                let can_pick = state.selected.is_some();
                if ui
                    .add_enabled(
                        can_pick,
                        egui::Button::new(
                            RichText::new(pick_label).color(theme.text_primary()),
                        ),
                    )
                    .clicked()
                {
                    if let Some(sel) = &state.selected {
                        result = FilePickerResult::Picked(sel.path.clone());
                    }
                }
                if widgets::Button::secondary("Cancel").show(ui, theme) {
                    result = FilePickerResult::Cancelled;
                }
            });
        });

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("hos_fb_test_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn lists_dirs_first_then_files_alphabetically() {
        let d = tmp("order");
        std::fs::create_dir(d.join("zeta_dir")).unwrap();
        std::fs::create_dir(d.join("alpha_dir")).unwrap();
        std::fs::write(d.join("b.txt"), "x").unwrap();
        std::fs::write(d.join("A.txt"), "x").unwrap();
        let names: Vec<String> = list_dir(&d, &[]).into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["alpha_dir", "zeta_dir", "A.txt", "b.txt"]);
    }

    #[test]
    fn extension_filter_keeps_dirs_and_matching_files_only() {
        let d = tmp("filter");
        std::fs::create_dir(d.join("sub")).unwrap();
        std::fs::write(d.join("model.STL"), "x").unwrap();
        std::fs::write(d.join("notes.txt"), "x").unwrap();
        std::fs::write(d.join("archive.tar.gz"), "x").unwrap();
        let names: Vec<String> = list_dir(&d, &["stl", "tar.gz"])
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert_eq!(names, vec!["sub", "archive.tar.gz", "model.STL"]);
    }

    #[test]
    fn hidden_dotfiles_are_skipped() {
        let d = tmp("hidden");
        std::fs::write(d.join(".secret"), "x").unwrap();
        std::fs::write(d.join("visible.txt"), "x").unwrap();
        let names: Vec<String> = list_dir(&d, &[]).into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["visible.txt"]);
    }

    #[test]
    fn size_formatting_is_readable() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2048), "2 KB");
        assert_eq!(human_size(3_250_585), "3.1 MB");
    }

    #[test]
    fn ext_matching_is_case_insensitive_and_dot_safe() {
        assert!(name_matches_ext("Photo.PNG", &["png"]));
        assert!(name_matches_ext("a.tar.gz", &["tar.gz"]));
        assert!(!name_matches_ext("notpng", &["png"]));
        assert!(!name_matches_ext("file.png.exe", &["png"]));
    }
}
