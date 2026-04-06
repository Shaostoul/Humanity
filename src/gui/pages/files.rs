//! File Browser page — browse data/ directory, view and edit text files.

use egui::{Color32, Frame, RichText, ScrollArea, Vec2};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;
use std::path::{Path, PathBuf};

/// A node in the directory tree.
#[derive(Debug, Clone)]
pub enum FsNode {
    Dir {
        name: String,
        path: PathBuf,
        children: Vec<FsNode>,
        expanded: bool,
    },
    File {
        name: String,
        path: PathBuf,
    },
}

/// Local state for the file browser.
pub struct FileBrowserState {
    pub root: Option<FsNode>,
    pub selected_file: Option<PathBuf>,
    pub file_content: String,
    pub file_dirty: bool,
    pub breadcrumbs: Vec<String>,
    pub status_message: String,
    pub initialized: bool,
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
        }
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
            expanded: depth == 0,
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

fn draw_tree(
    ui: &mut egui::Ui,
    node: &mut FsNode,
    theme: &Theme,
    selected: &mut Option<PathBuf>,
    content: &mut String,
    dirty: &mut bool,
    breadcrumbs: &mut Vec<String>,
    status: &mut String,
) {
    match node {
        FsNode::Dir {
            name,
            path,
            children,
            expanded,
        } => {
            let header = if *expanded {
                format!("v {}", name)
            } else {
                format!("> {}", name)
            };
            let resp = ui.selectable_label(
                false,
                RichText::new(&header).color(theme.accent()),
            );
            if resp.clicked() {
                *expanded = !*expanded;
            }
            if *expanded {
                ui.indent(egui::Id::new(path), |ui| {
                    for child in children.iter_mut() {
                        draw_tree(ui, child, theme, selected, content, dirty, breadcrumbs, status);
                    }
                });
            }
        }
        FsNode::File { name, path } => {
            let is_selected = selected.as_ref() == Some(path);
            let label_text = RichText::new(name.as_str()).color(if is_selected {
                theme.text_on_accent()
            } else {
                theme.text_primary()
            });
            let resp = ui.selectable_label(is_selected, label_text);
            if resp.clicked() {
                match std::fs::read_to_string(&*path) {
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
                        if s == "." || s == ".." {
                            None
                        } else {
                            Some(s)
                        }
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

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Initialize on first draw
    with_state(|fs| {
        if !fs.initialized {
            let data_path = PathBuf::from("data");
            if data_path.is_dir() {
                fs.root = scan_dir(&data_path, 0);
            }
            fs.initialized = true;
        }
    });

    egui::CentralPanel::default()
        .frame(Frame::none().fill(Color32::from_rgb(20, 20, 25)).inner_margin(16.0))
        .show(ctx, |ui| {
            ui.label(RichText::new("File Browser").size(theme.font_size_title).color(theme.text_primary()));
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
                            if let Some(ref mut root) = fs.root {
                                draw_tree(
                                    ui,
                                    root,
                                    theme,
                                    &mut fs.selected_file,
                                    &mut fs.file_content,
                                    &mut fs.file_dirty,
                                    &mut fs.breadcrumbs,
                                    &mut fs.status_message,
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
