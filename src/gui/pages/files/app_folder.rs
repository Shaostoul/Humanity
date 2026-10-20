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
//!
//! A LIBRARY GUIDE OPENS IN THE LIBRARY (2026-10-20; the operator chose on 2026-10-19 to keep the
//! Library a separate reading room and connect the two): a file in `data/library/` that the
//! Library catalogs (data/library/index.json, made from catalog.json) is found by [`guide_of`],
//! and "Open in the Library" (in the details, on a line over its text in the middle, and a
//! double-click on it in the tree) goes to the Library page with that guide open in its reader,
//! through the one function [`open_in_library`]. Its text can still be read and edited here.

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
/// The button and the link that open a Library guide in the Library.
pub const OPEN_IN_LIBRARY: &str = "Open in the Library";
/// Its hover words.
pub const OPEN_IN_LIBRARY_TIP: &str = "Read it in the Library, with its contents, its links and a dictionary for every word.";

/// A file in the HumanityOS folder that the Library catalogs: its slug and its title there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guide {
    pub slug: String,
    pub title: String,
}

/// One entry of a folder as read: its name, where it is, and whether it is a folder.
#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
}

/// The place's state, kept in `FilesState::app_folder` (kept when the page's server or person
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
    /// The file chosen is a guide the Library has ([`guide_of`]), found when it is chosen.
    pub guide: Option<Guide>,
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

/// The Library's own folder: `library` in the app's data folder, where the guides it reads are.
pub fn library_folder() -> PathBuf {
    let data = crate::data_dir();
    let data = if data.is_absolute() { data } else { std::env::current_dir().unwrap_or_default().join(data) };
    data.join("library")
}

/// The guide `path` is, when it is one the Library has: a file directly in `library_dir` whose
/// slug (worked out from its name as the Library works it out, loaders.rs
/// `library_slug_of_file`) is one of `library`'s documents. A file anywhere else, or one the
/// Library does not catalog (its index, a `.bak` copy), is none.
pub fn guide_of(path: &Path, library_dir: &Path, library: &[crate::gui::LibrarySection]) -> Option<Guide> {
    if path.parent()? != library_dir {
        return None;
    }
    let name = path.file_name()?.to_str()?;
    if !name.to_ascii_lowercase().ends_with(".md") {
        return None;
    }
    let slug = crate::gui::library_slug_of_file(name);
    library
        .iter()
        .flat_map(|s| s.categories.iter())
        .flat_map(|c| c.entries.iter())
        .find(|e| e.slug == slug)
        .map(|e| Guide { slug: e.slug.clone(), title: e.title.clone() })
}

/// "Open in the Library": go to the Library page with the guide `slug` open in its reader. The
/// one way this place opens a guide there (the details' button, the middle pane's link and a
/// double-click in the tree all come here). `false`, with the reason said, when the Library does
/// not have it.
pub(crate) fn open_in_library(state: &mut GuiState, slug: &str) -> bool {
    if crate::gui::pages::library::show_doc(state, slug) {
        state.push_nav_to(crate::gui::GuiPage::Library);
        true
    } else {
        state.files.app_folder.status = "The Library on this computer does not have this guide.".to_string();
        false
    }
}

/// Choose `path` (read it for the middle pane) and find whether it is a Library guide.
pub(crate) fn choose_in(state: &mut GuiState, path: &Path) {
    let guide = guide_of(path, &library_folder(), &state.library);
    let af = &mut state.files.app_folder;
    choose(af, path);
    af.guide = guide;
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
    let af = &mut state.files.app_folder;
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
    let mut opened: Option<PathBuf> = None;
    let AppFolder { tree, children, .. } = af;
    egui::ScrollArea::vertical().id_salt("app_folder_tree").auto_shrink([false, false]).show(ui, |ui| {
        let name = root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| root.display().to_string());
        draw_dir(ui, theme, tree, children, (&mut picked, &mut opened), &root, &name);
    });
    if let Some(path) = picked {
        choose_in(state, &path);
    }
    // A double-click on a Library guide opens it in the Library (the first click chose it).
    if let Some(path) = opened {
        if state.files.app_folder.selected.as_deref() != Some(path.as_path()) {
            choose_in(state, &path);
        }
        if let Some(guide) = state.files.app_folder.guide.clone() {
            open_in_library(state, &guide.slug);
        }
    }
}

/// One folder of the tree: its row, and when it is open, its entries (read once, kept). A file
/// clicked is `chosen.0`, one double-clicked `chosen.1`.
fn draw_dir(ui: &mut egui::Ui, theme: &Theme, tree: &mut widgets::TreeState, children: &mut HashMap<PathBuf, Vec<Entry>>, chosen: (&mut Option<PathBuf>, &mut Option<PathBuf>), dir: &Path, name: &str) {
    let id = dir.to_string_lossy().to_string();
    let (picked, opened) = chosen;
    widgets::tree_node(ui, theme, tree, &id, name, None, |ui, tree| {
        let entries = children.entry(dir.to_path_buf()).or_insert_with(|| read_folder(dir)).clone();
        if entries.is_empty() {
            ui.label(RichText::new("Nothing is in this folder.").size(theme.font_size_small).color(theme.text_muted()));
        }
        for e in &entries {
            if e.is_dir {
                draw_dir(ui, theme, tree, children, (&mut *picked, &mut *opened), &e.path, &e.name);
            } else {
                let leaf_id = e.path.to_string_lossy().to_string();
                let row = widgets::tree_leaf(ui, theme, tree, &leaf_id, &e.name, None);
                if row.clicked {
                    *picked = Some(e.path.clone());
                }
                if row.double_clicked {
                    *opened = Some(e.path.clone());
                }
            }
        }
    });
}

/// The middle pane: the file chosen, its text to read and edit, with Save.
pub(crate) fn draw_editor(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let af = &mut state.files.app_folder;
    let Some(path) = af.selected.clone() else {
        super::viewer::nothing_chosen(ui, theme, CHOOSE_LINE);
        return;
    };
    if let Some(why) = af.not_text.clone() {
        super::viewer::nothing_chosen(ui, theme, &why);
        return;
    }
    // A Library guide: a line over its text, with the link that opens it in the Library.
    let mut open_guide: Option<String> = None;
    if let Some(guide) = af.guide.clone() {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(guide_line(&guide)).size(theme.font_size_small).color(theme.text_secondary()));
            if ui.add(egui::Link::new(RichText::new(OPEN_IN_LIBRARY).size(theme.font_size_small).color(theme.accent()))).on_hover_text(OPEN_IN_LIBRARY_TIP).clicked() {
                open_guide = Some(guide.slug.clone());
            }
        });
        ui.add_space(theme.spacing_xs);
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
    if let Some(slug) = open_guide {
        open_in_library(state, &slug);
    }
}

/// The line over a Library guide's text in the middle pane.
pub fn guide_line(guide: &Guide) -> String {
    format!("A guide in the Library: {}.", guide.title.trim_end_matches('.'))
}

/// The right pane: where the file chosen is, its size, what editing it means, and Put back.
pub(crate) fn draw_details(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let af = &mut state.files.app_folder;
    let Some(path) = af.selected.clone() else {
        super::viewer::nothing_chosen(ui, theme, CAUTION);
        return;
    };
    let mut put = false;
    let mut open_guide: Option<String> = None;
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
        // A Library guide: read it in the Library, its reading room.
        if let Some(guide) = &af.guide {
            ui.label(RichText::new(&guide.title).size(theme.font_size_body).color(theme.text_primary()));
            if widgets::Button::primary(OPEN_IN_LIBRARY).tooltip(OPEN_IN_LIBRARY_TIP).show(ui, theme) {
                open_guide = Some(guide.slug.clone());
            }
            ui.add_space(theme.spacing_sm);
        }
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
    if let Some(slug) = open_guide {
        open_in_library(state, &slug);
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

    /// A guide the Library has, by its file in the Library's own folder.
    const GUIDE_FILE: &str = "starting_seeds.md";

    /// A LIBRARY GUIDE IS FOUND, AND OPENS IN THE LIBRARY (2026-10-20): a file in the Library's
    /// folder that the Library catalogs is a guide, with the Library's slug and title for it; a
    /// file it does not catalog (its index, a `.bak` copy, a Markdown file it does not list), or one
    /// of the same name in another folder, is not; `open_in_library` goes to the Library page with
    /// that guide open in its reader, and for a slug the Library does not have, stays and says why.
    ///
    /// Seen red 2026-10-20 with `open_in_library` going to the Library without opening the guide
    /// (`show_doc` not called, only `push_nav_to`): "that guide open in its reader" got None.
    #[test]
    fn a_library_guide_is_found_and_opens_in_the_library() {
        crate::config::keep_saves_off_disk();
        let mut state = GuiState::default();
        state.library = crate::gui::load_library(&crate::data_dir()).sections;
        let dir = library_folder();
        let guide = guide_of(&dir.join(GUIDE_FILE), &dir, &state.library).expect("a guide the Library has");
        assert_eq!((guide.slug.as_str(), guide.title.as_str()), ("starting-seeds", "Starting Seeds"));
        for not in [dir.join("index.json"), dir.join(format!("{GUIDE_FILE}.bak")), dir.join("not-in-the-library.md"), dir.parent().unwrap().join(GUIDE_FILE)] {
            assert_eq!(guide_of(&not, &dir, &state.library), None, "{not:?} is not a guide");
        }

        state.active_page = crate::gui::GuiPage::Files;
        assert!(open_in_library(&mut state, &guide.slug));
        assert_eq!(state.active_page, crate::gui::GuiPage::Library, "the Library page");
        assert_eq!(crate::gui::pages::library::shown_doc(&state).as_deref(), Some("starting-seeds"), "that guide open in its reader");

        state.active_page = crate::gui::GuiPage::Files;
        assert!(!open_in_library(&mut state, "no-such-guide"));
        assert_eq!(state.active_page, crate::gui::GuiPage::Files, "a guide the Library does not have: the page stays");
        assert!(state.files.app_folder.status.contains("does not have"), "and says why");
    }

    /// One frame of the Files page, `later` seconds after the last.
    fn page_frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, later: f64, events: Vec<egui::Event>) -> egui::FullOutput {
        let time = Some(ctx.input(|i| i.time) + later);
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 1000.0))), time, events, ..Default::default() };
        ctx.run(input, |ctx| super::super::draw(ctx, theme, state))
    }

    /// Where each `text` is drawn, left to right.
    fn drawn_at(out: &egui::FullOutput, text: &str) -> Vec<egui::Rect> {
        let mut all: Vec<egui::Rect> = crate::gui::pages::apps::tests::texts(out).into_iter().filter(|(t, _)| t == text).map(|(_, r)| r).collect();
        all.sort_by(|a, b| a.left().total_cmp(&b.left()));
        all
    }

    /// Press at `pos` (`clicks` times, a double-click when two).
    fn press(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, pos: egui::Pos2, clicks: usize) {
        let m = egui::Modifiers::default();
        page_frame(ctx, theme, state, 1.0, vec![egui::Event::PointerMoved(pos)]);
        for _ in 0..clicks {
            page_frame(ctx, theme, state, 0.05, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
            page_frame(ctx, theme, state, 0.05, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
        }
    }

    /// The Files page on the HumanityOS folder, its tree at the Library's own folder.
    fn on_the_library_folder() -> (egui::Context, Theme, GuiState) {
        let (ctx, theme) = super::super::tests::ctx_and_theme();
        let mut state = super::super::tests::page_state(super::super::tests::demo_answer(), true);
        state.library = crate::gui::load_library(&crate::data_dir()).sections;
        state.files.place = super::super::Place::AppFolder;
        state.files.app_folder.root = Some(library_folder());
        state.active_page = crate::gui::GuiPage::Files;
        (ctx, theme, state)
    }

    /// "OPEN IN THE LIBRARY" ON THE PAGE (2026-10-20): with a guide chosen, the details' button
    /// and the line over its text in the middle each open it in the Library (the same function);
    /// a double-click on a guide in the tree does too, its first click choosing it; a file that is
    /// not a guide has none of them, and a double-click on it stays on the Files page.
    ///
    /// Seen red 2026-10-20 with the tree's double-click not handled (`opened` never acted on in
    /// `draw_tree`): "a double-click on it in the tree" stayed on Files.
    #[test]
    fn open_in_the_library_on_the_page() {
        let (ctx, theme, mut state) = on_the_library_folder();
        let dir = library_folder();
        choose_in(&mut state, &dir.join(GUIDE_FILE));
        for _ in 0..3 {
            page_frame(&ctx, &theme, &mut state, 0.0, Vec::new());
        }
        let out = page_frame(&ctx, &theme, &mut state, 0.0, Vec::new());
        let opens = drawn_at(&out, OPEN_IN_LIBRARY);
        assert_eq!(opens.len(), 2, "the middle's link and the details' button");
        assert!(crate::gui::pages::apps::tests::exact(&out, &guide_line(state.files.app_folder.guide.as_ref().unwrap())), "the line over its text");
        press(&ctx, &theme, &mut state, opens[1].center(), 1);
        assert_eq!((state.active_page, crate::gui::pages::library::shown_doc(&state).as_deref()), (crate::gui::GuiPage::Library, Some("starting-seeds")), "the details' button");

        state.active_page = crate::gui::GuiPage::Files;
        crate::gui::pages::library::show_curriculum();
        // The link's words are laid out after the line's, in one row: press near their end.
        press(&ctx, &theme, &mut state, egui::pos2(opens[0].right() - 8.0, opens[0].center().y), 1);
        assert_eq!((state.active_page, crate::gui::pages::library::shown_doc(&state).as_deref()), (crate::gui::GuiPage::Library, Some("starting-seeds")), "the middle's link");

        // The tree: the Library's folder's first file, 00-START-HERE.md, is a guide too.
        state.active_page = crate::gui::GuiPage::Files;
        crate::gui::pages::library::show_curriculum();
        let out = page_frame(&ctx, &theme, &mut state, 0.0, Vec::new());
        let first = drawn_at(&out, "00-START-HERE.md").first().copied().expect("the first file in the tree");
        press(&ctx, &theme, &mut state, first.center(), 2);
        assert_eq!(state.active_page, crate::gui::GuiPage::Library, "a double-click on it in the tree");
        assert_eq!(crate::gui::pages::library::shown_doc(&state).as_deref(), Some("00-start-here"));

        // A file that is not a guide (here in a scratch folder): none of them, and a double-click
        // stays.
        let scratch = crate::test_temp::dir("app_folder_not_a_guide");
        std::fs::write(scratch.join("notes.txt"), "a note\n").unwrap();
        state.active_page = crate::gui::GuiPage::Files;
        state.files.app_folder.root = Some(scratch.to_path_buf());
        state.files.app_folder.children.clear();
        page_frame(&ctx, &theme, &mut state, 0.0, Vec::new());
        let out = page_frame(&ctx, &theme, &mut state, 0.0, Vec::new());
        let notes = drawn_at(&out, "notes.txt").first().copied().expect("notes.txt in the tree");
        press(&ctx, &theme, &mut state, notes.center(), 2);
        assert_eq!(state.files.app_folder.selected.as_deref(), Some(scratch.join("notes.txt").as_path()), "chosen");
        let out = page_frame(&ctx, &theme, &mut state, 0.0, Vec::new());
        assert_eq!(state.active_page, crate::gui::GuiPage::Files, "a file that is not a guide stays on the Files page");
        assert!(state.files.app_folder.guide.is_none() && drawn_at(&out, OPEN_IN_LIBRARY).is_empty(), "and has no Open in the Library");
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
