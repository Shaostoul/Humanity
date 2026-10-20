//! The Files page's "HumanityOS folder" place (2026-10-19): the app's own folder, the one that
//! holds `data/`, `assets/` and `mods/`, to look through and to read and edit its text files. It
//! was the developer's Files page (browse `data/`, view and edit text files) until the two pages
//! became one Files page; the operator, 2026-10-19: "If we just start at the root HumanityOS folder
//! for file browsing then we automatically have 'data' files and we even have mods at the top too."
//!
//! The folder is found the way the app finds its data (`crate::data_dir()`, `data` beside the app
//! unless set otherwise) and is the one above it. In the three panes: on the left, under the
//! places, the folder as a tree (a folder's contents are read when it is opened, never all at
//! once: in a developer's copy this folder holds the build output too); in the middle the file
//! chosen, its text to read and edit, with Save; on the right where it is, its size, and what
//! editing it means.
//!
//! EDITING CHANGES HOW THE APP WORKS, and the place says so in plain words. Save keeps the file's
//! previous version beside it as `<name>.bak` first (one copy: the version before the latest
//! save), and "Put back the previous version" brings that copy back. The app's data loaders read
//! files by their own endings (`.ron`, `.json`, `.csv`, ...), so a `.bak` copy is never read as data.
//! A file that is not text, or is larger than [`MAX_TEXT`], is described, not opened.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use egui::RichText;

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

/// The place's name in the places list.
pub const PLACE: &str = "HumanityOS folder";
/// What editing here means, said on the right and above the tree.
pub const CAUTION: &str = "These are the files HumanityOS runs on. Changing one changes how the app works. Save keeps the file's previous version beside it, ending in .bak, and Put back the previous version brings it back.";
/// The middle and right panes before a file is chosen.
pub const CHOOSE_LINE: &str = "Choose a file on the left to read it here.";
/// Put back's label.
pub const PUT_BACK: &str = "Put back the previous version";
/// The largest file shown as text.
pub const MAX_TEXT: u64 = 2 * 1024 * 1024;

/// One entry of a folder as read: its name, where it is, and whether it is a folder.
#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
}

/// The place's state, kept in `MediaState::app_folder` (kept when the page's server or person
/// changes: nothing in it belongs to a server).
#[derive(Default)]
pub struct AppFolder {
    /// The folder at the top of the tree, found on first view.
    pub root: Option<PathBuf>,
    /// Each folder opened so far, as last read (read again on Refresh).
    pub children: HashMap<PathBuf, Vec<Entry>>,
    /// The tree's open folders and its chosen file.
    pub tree: widgets::TreeState,
    /// The file chosen, and its text as edited.
    pub selected: Option<PathBuf>,
    pub content: String,
    /// The text was changed since it was read or saved.
    pub dirty: bool,
    /// Why the file chosen is not shown as text (not text, or too large), when it is not.
    pub not_text: Option<String>,
    /// One line about the last thing done.
    pub status: String,
}

/// The app's own folder: the one above its data folder.
pub fn root_folder() -> PathBuf {
    let data = crate::data_dir();
    let data = if data.is_absolute() { data } else { std::env::current_dir().unwrap_or_default().join(data) };
    match data.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => std::env::current_dir().unwrap_or_default(),
    }
}

/// A folder's entries, folders first, then by name.
pub fn read_folder(dir: &Path) -> Vec<Entry> {
    let mut out: Vec<Entry> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| {
            let meta = e.metadata().ok();
            Entry {
                name: e.file_name().to_string_lossy().to_string(),
                path: e.path(),
                is_dir: meta.as_ref().is_some_and(|m| m.is_dir()),
                size: meta.as_ref().map_or(0, |m| m.len()),
            }
        })
        .collect();
    out.sort_by(|a, b| (!a.is_dir, a.name.to_lowercase()).cmp(&(!b.is_dir, b.name.to_lowercase())));
    out
}

/// Where a file's previous version is kept: beside it, `<name>.bak`.
pub fn backup_of(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".bak");
    path.with_file_name(name)
}

/// Read `path` for the middle pane: its text, or why it is not shown as text.
pub fn read_text(path: &Path) -> Result<String, String> {
    let len = std::fs::metadata(path).map_err(|e| format!("This file could not be read: {e}"))?.len();
    if len > MAX_TEXT {
        return Err(format!(
            "This file is {}, larger than this page opens as text ({} at most).",
            widgets::file_browser::human_size(len),
            widgets::file_browser::human_size(MAX_TEXT)
        ));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("This file could not be read: {e}"))?;
    String::from_utf8(bytes).map_err(|_| "This file is not text, so it is not opened here.".to_string())
}

/// Save `text` to `path`, its previous version kept beside it first ([`backup_of`]). The words
/// to show.
pub fn save(path: &Path, text: &str) -> Result<String, String> {
    if path.is_file() {
        std::fs::copy(path, backup_of(path)).map_err(|e| format!("Not saved: the previous version could not be kept ({e})."))?;
    }
    std::fs::write(path, text).map_err(|e| format!("Not saved: {e}"))?;
    Ok("Saved. The previous version is kept beside it, ending in .bak.".to_string())
}

/// Put the previous version (`<name>.bak`) back over `path`. The words to show.
pub fn put_back(path: &Path) -> Result<String, String> {
    let bak = backup_of(path);
    if !bak.is_file() {
        return Err("There is no previous version of this file to put back.".to_string());
    }
    std::fs::copy(&bak, path).map_err(|e| format!("Not put back: {e}"))?;
    Ok("The previous version is back.".to_string())
}

/// Choose `path`: read it for the middle pane.
pub(crate) fn choose(af: &mut AppFolder, path: &Path) {
    af.selected = Some(path.to_path_buf());
    af.dirty = false;
    af.status.clear();
    match read_text(path) {
        Ok(text) => {
            af.content = text;
            af.not_text = None;
        }
        Err(why) => {
            af.content.clear();
            af.not_text = Some(why);
        }
    }
}

/// The left pane's part, under the places: Refresh, then the folder as a tree.
pub(crate) fn draw_tree(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let af = &mut state.media.app_folder;
    let root = af.root.get_or_insert_with(root_folder).clone();
    if !af.tree.is_expanded(&root.to_string_lossy()) && af.children.is_empty() {
        af.tree.expand(&root.to_string_lossy());
    }
    widgets::subsection_label(ui, theme, PLACE);
    ui.horizontal_wrapped(|ui| {
        if widgets::Button::ghost("Refresh").tooltip("Read the folders again.").show(ui, theme) {
            af.children.clear();
        }
        ui.label(RichText::new(root.display().to_string()).size(theme.font_size_small).color(theme.text_muted()));
    });
    let mut picked: Option<PathBuf> = None;
    let AppFolder { tree, children, .. } = af;
    egui::ScrollArea::vertical().id_salt("app_folder_tree").auto_shrink([false, false]).show(ui, |ui| {
        let name = root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| root.display().to_string());
        draw_dir(ui, theme, tree, children, &mut picked, &root, &name);
    });
    if let Some(path) = picked {
        choose(&mut state.media.app_folder, &path);
    }
}

/// One folder of the tree: its row, and when it is open, its entries (read once, kept).
fn draw_dir(ui: &mut egui::Ui, theme: &Theme, tree: &mut widgets::TreeState, children: &mut HashMap<PathBuf, Vec<Entry>>, picked: &mut Option<PathBuf>, dir: &Path, name: &str) {
    let id = dir.to_string_lossy().to_string();
    widgets::tree_node(ui, theme, tree, &id, name, None, |ui, tree| {
        let entries = children.entry(dir.to_path_buf()).or_insert_with(|| read_folder(dir)).clone();
        if entries.is_empty() {
            ui.label(RichText::new("Nothing is in this folder.").size(theme.font_size_small).color(theme.text_muted()));
        }
        for e in &entries {
            if e.is_dir {
                draw_dir(ui, theme, tree, children, picked, &e.path, &e.name);
            } else {
                let leaf_id = e.path.to_string_lossy().to_string();
                if widgets::tree_leaf(ui, theme, tree, &leaf_id, &e.name, None).clicked {
                    *picked = Some(e.path.clone());
                }
            }
        }
    });
}

/// The middle pane: the file chosen, its text to read and edit, with Save.
pub(crate) fn draw_editor(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let af = &mut state.media.app_folder;
    let Some(path) = af.selected.clone() else {
        super::viewer::nothing_chosen(ui, theme, CHOOSE_LINE);
        return;
    };
    if let Some(why) = af.not_text.clone() {
        super::viewer::nothing_chosen(ui, theme, &why);
        return;
    }
    let mut save_it = false;
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
        // The row at the foot, sized as panes.rs `foot_row` sizes the middle pane's rows.
        super::panes::foot_row(ui, egui::Id::new("app_folder_save_row"), false, |ui| {
            save_it = ui.add_enabled_ui(af.dirty, |ui| widgets::Button::primary("Save").tooltip("Write the change to the file. Its previous version is kept beside it.").show(ui, theme)).inner;
            if !af.status.is_empty() {
                ui.label(RichText::new(&af.status).size(theme.font_size_small).color(theme.text_muted()));
            }
        });
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            // As many rows as the pane holds, so the whole pane is the place to click and type.
            let rows = (ui.available_height() / (theme.font_size_body * 1.35)).max(4.0) as usize;
            egui::ScrollArea::both().id_salt("app_folder_text").auto_shrink([false, false]).show(ui, |ui| {
                let resp = ui.add(egui::TextEdit::multiline(&mut af.content).desired_width(f32::INFINITY).desired_rows(rows).font(egui::TextStyle::Monospace));
                if resp.changed() {
                    af.dirty = true;
                }
            });
        });
    });
    if save_it {
        match save(&path, &af.content) {
            Ok(words) => {
                af.dirty = false;
                af.status = words;
            }
            Err(words) => af.status = words,
        }
    }
}

/// The right pane: where the file chosen is, its size, what editing it means, and Put back.
pub(crate) fn draw_details(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let af = &mut state.media.app_folder;
    let Some(path) = af.selected.clone() else {
        super::viewer::nothing_chosen(ui, theme, CAUTION);
        return;
    };
    let mut put = false;
    egui::ScrollArea::vertical().id_salt("app_folder_details").auto_shrink([false, false]).show(ui, |ui| {
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        ui.horizontal_wrapped(|ui| {
            widgets::file_badge::show(ui, theme, &crate::net::file_risk::file_type(&name, None));
            ui.label(RichText::new(&name).size(theme.font_size_heading).strong().color(theme.text_primary()));
        });
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        ui.label(RichText::new(widgets::file_browser::human_size(size)).size(theme.font_size_small).color(theme.text_secondary()));
        ui.label(RichText::new(path.display().to_string()).size(theme.font_size_small).color(theme.text_muted()));
        ui.add_space(theme.spacing_sm);
        widgets::body_hint(ui, theme, CAUTION);
        if af.dirty {
            ui.label(RichText::new("Changed, not saved yet.").size(theme.font_size_small).color(theme.warning()));
        }
        if backup_of(&path).is_file() {
            ui.add_space(theme.spacing_sm);
            put = widgets::Button::secondary(PUT_BACK).tooltip("Bring back the version this file had before its latest save.").show(ui, theme);
        }
    });
    if put {
        match put_back(&path) {
            Ok(words) => {
                choose(af, &path);
                af.status = words;
            }
            Err(words) => af.status = words,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SAVE KEEPS THE PREVIOUS VERSION, AND PUT BACK BRINGS IT BACK (2026-10-19): editing the app's
    /// own files changes how it works, so a save is never the only copy.
    #[test]
    fn save_keeps_the_previous_version_and_put_back_brings_it_back() {
        let dir = crate::test_temp::dir("app_folder_save");
        let file = dir.join("plants.csv");
        std::fs::write(&file, "id,name\n1,bean\n").unwrap();
        assert!(put_back(&file).is_err(), "no previous version before the first save");
        save(&file, "id,name\n1,pea\n").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "id,name\n1,pea\n", "saved");
        assert_eq!(std::fs::read_to_string(backup_of(&file)).unwrap(), "id,name\n1,bean\n", "the previous version beside it");
        assert_eq!(backup_of(&file).file_name().unwrap().to_string_lossy(), "plants.csv.bak");
        put_back(&file).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "id,name\n1,bean\n", "and back");
    }

    /// A FILE THAT IS NOT TEXT, OR TOO LARGE, IS DESCRIBED, NOT OPENED.
    #[test]
    fn a_file_that_is_not_text_is_not_opened() {
        let dir = crate::test_temp::dir("app_folder_text");
        let bin = dir.join("icon.png");
        std::fs::write(&bin, [0x89u8, 0x50, 0x4e, 0x47, 0xff, 0xfe, 0x00]).unwrap();
        assert!(read_text(&bin).unwrap_err().contains("not text"));
        let text = dir.join("notes.md");
        std::fs::write(&text, "# Notes\n").unwrap();
        assert_eq!(read_text(&text).unwrap(), "# Notes\n");
    }

    /// THE FOLDER IS THE ONE ABOVE THE DATA FOLDER, read folders first.
    #[test]
    fn the_folder_is_the_one_above_data_and_folders_come_first() {
        let root = root_folder();
        assert!(root.join("data").is_dir() || crate::data_dir().is_absolute(), "the app's folder holds data/: {root:?}");
        let dir = crate::test_temp::dir("app_folder_order");
        std::fs::create_dir_all(dir.join("zeta")).unwrap();
        std::fs::write(dir.join("alpha.txt"), "a").unwrap();
        let names: Vec<String> = read_folder(&dir).into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["zeta".to_string(), "alpha.txt".to_string()], "folders first");
    }
}
