//! The Media page's "Your folders" tab (2026-10-18; the operator, docs/PRIORITIES.md "Next for
//! Media": "point HumanityOS at folders (his E: 'Media' drive), browse them like a file explorer,
//! pick files to put on a server's Media page; nothing leaves the computer unless picked").
//!
//! DESKTOP ONLY. The website has no Your folders (operator, 2026-10-13: no website version, so
//! every browser behaves the same: a web page cannot be pointed at a folder and look through it the
//! same way in every browser).
//!
//! What it has, top to bottom:
//! - YOUR FOLDERS: "Add a folder" opens the app's folder picker (widgets/file_browser.rs); each
//!   folder added is a row with its type's badge, its name, its path and Remove, which only forgets
//!   it here (nothing on the disk is touched). The list is kept with the app's other settings
//!   (`settings.media_folders`, saved in config.json through `settings_dirty`) and survives a
//!   restart. A folder that is not there right now (a drive unplugged) says so in its row and is
//!   not opened.
//! - THE FOLDER OPEN: its contents in the file picker's own views, List, Details and Pictures with
//!   the across slider (widgets/file_views.rs; which view and how many across are the pickers' own,
//!   remembered on this computer), every entry with its type's badge, pictures (and on Windows
//!   videos) with previews (widgets/thumbnails.rs). Sub-folders open in place; Up goes back but
//!   never above the folder added, so the tab stays inside the folders the person chose.
//! - CHOOSING: a click on a file ticks or unticks it (a folder opens instead); "N chosen" with
//!   Clear, and "Put on Media", which hands the ticked files to Add to Media exactly as dropping
//!   them on the window does (add.rs `upload_dropped`: the first ticked is the item, the rest its
//!   added files) and goes to the Media tab, where its form is open. NOTHING IS SENT until the
//!   person presses Upload on that form.
//! - MARKS: beside a file this computer already put on a server's Media page, "On
//!   united-humanity.us as Dusk" (or "On Media" when the item's title is not known), a link that
//!   opens the item. From the private record kept on this computer (from_here.rs), only while the
//!   file is the same size and date it was when picked.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui::RichText;

use super::from_here::{self, Sent};
use super::MediaTab;
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::widgets::file_browser::{file_picker_modal, human_size, list_dir, FilePickerResult, FilePickerState, FsEntry};
use crate::gui::widgets::file_picker_memory::{self as memory, same_folder, PickerView};
use crate::gui::widgets::file_views::{self, Act, Look, Notes, SortBy};
use crate::gui::widgets::tint_section::{self, Tint};
use crate::gui::GuiState;

/// The tab's name.
pub const TAB: &str = "Your folders";
/// The button that adds a folder, and the folder picker's title.
pub const ADD_FOLDER: &str = "Add a folder";
pub const PICKER_TITLE: &str = "Your folders: choose a folder to add";
/// A folder row's button that forgets it here.
pub const REMOVE: &str = "Remove";
/// Back to the folder above, never above the folder added.
pub const UP: &str = "Up";
/// Untick every file.
pub const CLEAR: &str = "Clear";
/// Hand the ticked files to Add to Media.
pub const PUT_ON_MEDIA: &str = "Put on Media";
/// The tab's promise, under the page's heading.
pub const HINT: &str = "Folders on this computer, to look through and pick from. Nothing in them leaves this computer unless you tick it, press Put on Media, and then press Upload.";
/// With no folder added yet.
pub const NONE_YET: &str = "No folders yet. Press Add a folder to choose one, such as a drive of pictures and videos.";
/// A folder that is not there right now (the file picker's own words).
pub const MISSING: &str = "This folder is not there right now: its drive may be unplugged, or the folder moved or deleted.";
/// With folders added and none open.
pub const PICK_ONE: &str = "Click one of your folders to see what is in it.";
/// How choosing works, above the folder's files.
pub const HOW_TO_PICK: &str = "Click files to tick them, then press Put on Media. The first one ticked is the item; the others are added to it.";
/// Nothing ticked yet.
pub const NOTHING_CHOSEN: &str = "Nothing chosen yet.";
/// Why Put on Media cannot go: no server, or not signed in to it.
pub const NEED_SERVER: &str = "To put files on a Media page, connect to a server first, on the Chat page.";
pub const NEED_SIGN_IN: &str = "To put files on this server's Media page, sign in to it first, on the Chat page.";
/// An open folder with nothing in it.
pub const EMPTY_FOLDER: &str = "Nothing is in this folder.";
/// The Details view's column of marks.
pub const MARKS_HEADING: &str = "On Media";
/// A mark whose item's title is not known.
pub const ON_MEDIA: &str = "On Media";
/// At most this many folders are kept here.
pub const MAX_FOLDERS: usize = 100;
/// How long the open folder's listing is reused before it is read again: a file changed meanwhile
/// (its mark then goes) or added shows within this.
const RELIST: Duration = Duration::from_secs(1);

/// The tab's state, kept in `MediaState::folders` (and kept when the page's server or person
/// changes: nothing in it belongs to a server).
#[derive(Default)]
pub struct Browse {
    /// The added folder open (one of `settings.media_folders`), and the folder shown inside it.
    pub root: Option<PathBuf>,
    pub dir: PathBuf,
    /// The files ticked, in the order ticked: they stay ticked from folder to folder.
    pub chosen: Vec<FsEntry>,
    /// "Add a folder"'s folder picker, while open.
    pub picker: Option<FilePickerState>,
    /// The Details view's sort.
    pub sort: SortBy,
    pub sort_desc: bool,
    /// One line about the last thing refused here.
    pub status: String,
    /// The open folder as last read, with its marks.
    listing: Option<Listing>,
    /// When (the frame's input time) a click last opened a sub-folder: see [`draw_browser`].
    opened_at: Option<f64>,
}

/// The open folder as last read: its entries, and each marked file's marks (and their words).
struct Listing {
    dir: PathBuf,
    at: Instant,
    /// The record's generation it was marked from: a publish finishing marks at once.
    generation: u64,
    entries: Vec<FsEntry>,
    marks: HashMap<PathBuf, Vec<Sent>>,
    notes: Notes,
}

/// A mark's words: "On united-humanity.us as Dusk", or "On Media" when the title is not known.
pub fn mark_words(s: &Sent) -> String {
    if s.title.trim().is_empty() {
        ON_MEDIA.to_string()
    } else {
        format!("On {} as {}", crate::gui::pages::server_strip::host_of(&s.server), s.title.trim())
    }
}

/// A mark's hover words.
fn mark_tip(s: &Sent) -> String {
    format!("This file was put on {}'s Media page from this computer. Click to open it.", crate::gui::pages::server_strip::host_of(&s.server))
}

/// "2 chosen (3.1 MB)".
pub fn chosen_words(chosen: &[FsEntry]) -> String {
    let total: u64 = chosen.iter().map(|c| c.size).sum();
    format!("{} chosen ({})", chosen.len(), human_size(total))
}

/// Read the open folder again when it changed, a publish added a mark, or [`RELIST`] passed.
fn refresh(b: &mut Browse) {
    let generation = from_here::with(|r| r.generation);
    if b.listing.as_ref().is_some_and(|l| l.dir == b.dir && l.generation == generation && l.at.elapsed() < RELIST) {
        return;
    }
    let entries = list_dir(&b.dir, &[]);
    let marks = from_here::with(|r| r.marks_for(&entries));
    let notes = marks.iter().map(|(p, rows)| (p.clone(), rows.iter().map(|s| (mark_words(s), mark_tip(s))).collect())).collect();
    b.listing = Some(Listing { dir: b.dir.clone(), at: Instant::now(), generation, entries, marks, notes });
}

/// Open `dir` (one of the folders added) in the browser.
fn open_folder(b: &mut Browse, dir: &Path) {
    b.root = Some(dir.to_path_buf());
    b.dir = dir.to_path_buf();
    b.listing = None;
}

/// Add `dir` to the folders (once; any spelling of a folder already there is that folder), kept
/// with the settings, and open it.
pub(crate) fn add_folder(state: &mut GuiState, dir: &Path) {
    if !state.settings.media_folders.iter().any(|f| same_folder(f, dir)) {
        if state.settings.media_folders.len() >= MAX_FOLDERS {
            state.media.folders.status = format!("At most {MAX_FOLDERS} folders can be added here. Remove one first.");
            return;
        }
        let mut folders = state.settings.media_folders.clone();
        folders.push(dir.to_path_buf());
        state.settings.media_folders = folders;
        state.settings_dirty = true;
    }
    state.media.folders.status.clear();
    open_folder(&mut state.media.folders, dir);
}

/// Forget `dir` here (nothing on the disk is touched): out of the list and the settings; its files
/// unticked, and closed if it was open.
pub(crate) fn remove_folder(state: &mut GuiState, dir: &Path) {
    let mut folders = state.settings.media_folders.clone();
    folders.retain(|f| !same_folder(f, dir));
    state.settings.media_folders = folders;
    state.settings_dirty = true;
    let b = &mut state.media.folders;
    if b.root.as_deref().is_some_and(|r| same_folder(r, dir)) {
        b.root = None;
        b.listing = None;
    }
    b.chosen.retain(|c| !c.path.starts_with(dir));
}

/// Up: the folder above, but never above the folder added.
fn up(b: &mut Browse) {
    let Some(root) = b.root.clone() else { return };
    if same_folder(&b.dir, &root) {
        return;
    }
    b.dir = match b.dir.parent() {
        Some(p) if p.starts_with(&root) => p.to_path_buf(),
        _ => root,
    };
}

/// Why Put on Media cannot go now, if it cannot.
pub(crate) fn why_not_put(state: &GuiState) -> Option<&'static str> {
    if super::server_here(state).is_empty() {
        Some(NEED_SERVER)
    } else if !super::signed_in_here(state) {
        Some(NEED_SIGN_IN)
    } else {
        None
    }
}

/// Put on Media: the ticked files go to Add to Media as dropped files do (the first is the item,
/// the rest added to it once it is up), and the Media tab, where its form is open, is shown.
/// Nothing is uploaded until Upload is pressed there. The ticks go once the form has them.
pub(crate) fn put_on_media(state: &mut GuiState) {
    let paths: Vec<PathBuf> = state.media.folders.chosen.iter().map(|c| c.path.clone()).collect();
    if paths.is_empty() {
        return;
    }
    if let Some(why) = why_not_put(state) {
        state.media.folders.status = why.to_string();
        return;
    }
    super::add::upload_dropped(state, &paths, None);
    if state.media.form.is_some() {
        state.media.folders.chosen.clear();
        state.media.folders.status.clear();
    }
    state.media.tab = MediaTab::Media;
}

/// A mark was clicked: its item opens on the Media page, when it is on the server this page is
/// for; otherwise the line says where it is.
fn open_mark(state: &mut GuiState, s: &Sent) {
    if s.server != super::server_here(state) {
        let host = crate::gui::pages::server_strip::host_of(&s.server);
        state.media.folders.status = format!("That is on {host}. Connect to it on the Chat page to open it there.");
        return;
    }
    if s.item > 0 {
        super::open_item_by_number(state, s.item);
    } else {
        state.media.tab = MediaTab::Media;
    }
}

/// The folder picker of "Add a folder", while open (drawn over the page).
pub(crate) fn draw_dialogs(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let Some(mut picker) = state.media.folders.picker.take() else { return };
    match file_picker_modal(ctx, theme, &mut picker, PICKER_TITLE) {
        FilePickerResult::Open => state.media.folders.picker = Some(picker),
        FilePickerResult::Cancelled => {}
        FilePickerResult::Picked(paths) => {
            if let Some(dir) = paths.first() {
                add_folder(state, dir);
            }
        }
    }
}

/// The tab, inside the page's scroll area.
pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::thumbnails::next_frame();
    widgets::body_hint(ui, theme, HINT);
    if !state.media.folders.status.is_empty() {
        ui.label(RichText::new(&state.media.folders.status).size(theme.font_size_small).color(theme.warning()));
    }
    ui.add_space(theme.spacing_sm);
    draw_folder_list(ui, theme, state);
    ui.add_space(theme.spacing_sm);
    draw_browser(ui, theme, state);
}

/// The folders added, each with its name, path and Remove; "Add a folder" above them.
fn draw_folder_list(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let folders = state.settings.media_folders.clone();
    let open_root = state.media.folders.root.clone();
    let (mut add, mut open, mut remove) = (false, None, None);
    tint_section::show(ui, theme, Tint::Red, TAB, |ui| {
        add = widgets::Button::primary(ADD_FOLDER).tooltip("Choose a folder on this computer to look through here. Nothing in it is sent anywhere.").show(ui, theme);
        if folders.is_empty() {
            ui.label(RichText::new(NONE_YET).size(theme.font_size_small).color(theme.text_muted()));
        }
        for (label, path) in memory::favorite_labels(&folders) {
            let here = path.is_dir();
            let is_open = open_root.as_deref().is_some_and(|r| same_folder(r, &path));
            ui.horizontal_wrapped(|ui| {
                widgets::file_badge::show(ui, theme, &widgets::file_badge::folder());
                if here {
                    let name = RichText::new(&label).size(theme.font_size_body).strong().color(theme.text_primary());
                    if ui.selectable_label(is_open, name).on_hover_text("Show what is in it.").clicked() {
                        open = Some(path.clone());
                    }
                } else {
                    ui.label(RichText::new(&label).size(theme.font_size_body).color(theme.text_muted()));
                }
                ui.label(RichText::new(path.display().to_string()).size(theme.font_size_small).color(theme.text_muted()));
                if widgets::Button::secondary(REMOVE).size(widgets::ButtonSize::Small).tooltip("Take it off this list. Nothing on your computer is changed.").show(ui, theme) {
                    remove = Some(path.clone());
                }
            });
            if !here {
                ui.label(RichText::new(MISSING).size(theme.font_size_small).color(theme.warning()));
            }
        }
    });
    if add {
        state.media.folders.picker = Some(FilePickerState::new_dir_picker(None));
    }
    if let Some(dir) = open {
        open_folder(&mut state.media.folders, &dir);
    }
    if let Some(dir) = remove {
        remove_folder(state, &dir);
    }
}

/// The folder open: Up and where it is, what is chosen with Clear and Put on Media, the views'
/// buttons, then its contents.
fn draw_browser(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let Some(root) = state.media.folders.root.clone() else {
        if !state.settings.media_folders.is_empty() {
            ui.label(RichText::new(PICK_ONE).size(theme.font_size_small).color(theme.text_muted()));
        }
        return;
    };
    let folders = state.settings.media_folders.clone();
    let title = memory::favorite_labels(&folders).into_iter().find(|(_, p)| same_folder(p, &root)).map(|(l, _)| l).unwrap_or_else(|| root.display().to_string());
    if !root.is_dir() {
        tint_section::show(ui, theme, Tint::Blue, &title, |ui| {
            ui.label(RichText::new(MISSING).size(theme.font_size_small).color(theme.warning()));
        });
        return;
    }
    // A sub-folder gone since it was opened: back to the folder added.
    if !state.media.folders.dir.is_dir() || !state.media.folders.dir.starts_with(&root) {
        state.media.folders.dir = root.clone();
    }
    refresh(&mut state.media.folders);
    let why_not = why_not_put(state);
    let b = &mut state.media.folders;
    let Some(listing) = b.listing.take() else { return };
    let at_top = same_folder(&b.dir, &root);
    let (mut go_up, mut clear, mut put, mut act) = (false, false, false, None);
    let (mut view, mut across) = memory::with(|m| (m.view, m.across.unwrap_or(4)));
    tint_section::show(ui, theme, Tint::Blue, &title, |ui| {
        ui.horizontal_wrapped(|ui| {
            go_up = ui.add_enabled_ui(!at_top, |ui| widgets::Button::secondary(UP).tooltip("Back to the folder above, within this one.").show(ui, theme)).inner;
            ui.label(RichText::new(b.dir.display().to_string()).size(theme.font_size_small).color(theme.text_muted()));
        });
        ui.add_space(theme.spacing_xs);
        ui.horizontal_wrapped(|ui| {
            if b.chosen.is_empty() {
                ui.label(RichText::new(NOTHING_CHOSEN).size(theme.font_size_body).color(theme.text_secondary()));
            } else {
                let names: Vec<&str> = b.chosen.iter().map(|c| c.name.as_str()).collect();
                ui.label(RichText::new(chosen_words(&b.chosen)).size(theme.font_size_body).strong().color(theme.text_primary())).on_hover_text(names.join("\n"));
                clear = widgets::Button::secondary(CLEAR).tooltip("Untick every file.").show(ui, theme);
            }
            let can = !b.chosen.is_empty() && why_not.is_none();
            put = ui
                .add_enabled_ui(can, |ui| widgets::Button::primary(PUT_ON_MEDIA).size(widgets::ButtonSize::Large).tooltip("Open Add to Media with these files. Nothing is sent until you press Upload there.").show(ui, theme))
                .inner;
        });
        if let (false, Some(why)) = (b.chosen.is_empty(), why_not) {
            ui.label(RichText::new(why).size(theme.font_size_small).color(theme.warning()));
        }
        ui.label(RichText::new(HOW_TO_PICK).size(theme.font_size_small).color(theme.text_secondary()));
        ui.add_space(theme.spacing_xs);
        if file_views::toolbar(ui, theme, &mut view, &mut across) {
            memory::with(|m| {
                m.view = view;
                m.across = Some(across);
            });
        }
        ui.add_space(theme.spacing_xs);
        if listing.entries.is_empty() {
            ui.label(RichText::new(EMPTY_FOLDER).size(theme.font_size_small).color(theme.text_muted()));
        }
        let look = Look { selected: None, chosen: &b.chosen, multiple: true, dir_mode: false, max_size: 0, notes: Some((MARKS_HEADING, &listing.notes)) };
        act = match view {
            PickerView::List => file_views::list(ui, theme, &listing.entries, &look),
            PickerView::Details => file_views::details(ui, theme, &listing.entries, &look, &mut b.sort, &mut b.sort_desc),
            PickerView::Pictures => file_views::pictures(ui, theme, &listing.entries, &look, across),
        };
    });
    // A DOUBLE-CLICK ON A FOLDER (the habit other file explorers teach) opens it with its first
    // click, and its second lands on whatever the folder just opened shows under the pointer: that
    // second click is ignored, so it never ticks a file or opens a folder nobody aimed at.
    let now = ui.input(|i| i.time);
    let delay = ui.ctx().options(|o| o.input_options.max_double_click_delay);
    let b = &mut state.media.folders;
    let act = if b.opened_at.is_some_and(|t| now - t < delay) { None } else { act };
    let mark = match &act {
        Some(Act::Note(path, i)) => listing.marks.get(path).and_then(|m| m.get(*i)).cloned(),
        _ => None,
    };
    b.listing = Some(listing);
    match act {
        Some(Act::Open(dir)) if dir.starts_with(&root) => {
            b.dir = dir;
            b.opened_at = Some(now);
        }
        Some(Act::Toggle(e)) => {
            if b.chosen.iter().any(|c| c.path == e.path) {
                b.chosen.retain(|c| c.path != e.path);
            } else {
                b.chosen.push(e);
            }
        }
        _ => {}
    }
    if go_up {
        up(b);
    }
    if clear {
        b.chosen.clear();
    }
    if let Some(s) = mark {
        open_mark(state, &s);
    }
    if put {
        put_on_media(state);
    }
}

#[cfg(test)]
#[path = "your_folders_tests.rs"]
mod tests;
