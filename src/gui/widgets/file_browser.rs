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
use super::tint_section::Tint;
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::widgets::file_picker_memory as memory;
use egui::RichText;

/// One entry in a directory listing.
#[derive(Debug, Clone, PartialEq)]
pub struct FsEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    /// When it was last changed, when the system says (the Details view's date, and its sort).
    pub modified: Option<std::time::SystemTime>,
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
                dirs.push(FsEntry { name, path, is_dir: true, size: 0, modified: meta.modified().ok() });
            } else {
                if !allowed_exts.is_empty() && !name_matches_ext(&name, allowed_exts) {
                    continue;
                }
                files.push(FsEntry { name, path, is_dir: false, size: meta.len(), modified: meta.modified().ok() });
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

/// How long a disc-drive answer is reused before the machine is asked
/// again. The picker redraws every frame and asking an optical drive
/// whether it holds a disc can spin it up, so the answer is held for a
/// moment; two seconds is short enough that a disc pushed in while the
/// picker is open still appears on its own.
const DISC_ROOT_REFRESH: std::time::Duration = std::time::Duration::from_secs(2);

/// Every drive a person can browse right now, cached for
/// `DISC_ROOT_REFRESH`. The list itself comes from
/// `media::dvd::drive_roots`: internal drives, USB sticks and memory cards,
/// and an optical drive while it holds a disc.
fn drive_roots_cached() -> Vec<(String, PathBuf)> {
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<Option<(std::time::Instant, Vec<(String, PathBuf)>)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = cache.lock().unwrap();
    if let Some((asked, roots)) = guard.as_ref() {
        if asked.elapsed() < DISC_ROOT_REFRESH {
            return roots.clone();
        }
    }
    let roots: Vec<(String, PathBuf)> =
        crate::media::dvd::drive_roots().into_iter().map(|(label, path, _)| (label, path)).collect();
    *guard = Some((std::time::Instant::now(), roots.clone()));
    roots
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
        for (label, sub) in [
            ("Home", ""),
            ("Downloads", "Downloads"),
            ("Documents", "Documents"),
            ("Desktop", "Desktop"),
            // Where a person's own films live; the in-world video screens'
            // Open picker starts here (2026-09-18).
            ("Videos", "Videos"),
        ] {
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
    // Every drive on the machine, so a picker can reach one without anyone
    // typing a drive letter, which this picker has nowhere to do
    // (2026-09-18). The operator hit this the moment the video screens
    // shipped: his films live on E: and nothing in the row could reach it.
    // A drive with nothing in it, an empty card reader or an optical drive
    // with no disc, is simply not offered.
    roots.extend(drive_roots_cached());
    roots
}

/// The folder a picker may confirm AS A WHOLE, when it is looking at one.
/// `markers` are directory names that mean "this folder is the thing"
/// (`VIDEO_TS` for a video disc): the offer stands when the open folder IS
/// one of them or HOLDS one, and the path returned is the open folder
/// either way, because both are what a disc player is handed.
///
/// `None` (no offer) for everything else, so an ordinary picker is
/// unchanged and nobody can confirm a folder where a file is wanted.
pub fn folder_offer(dir: &Path, markers: &[String]) -> Option<PathBuf> {
    if markers.is_empty() || !dir.is_dir() {
        return None;
    }
    let matches = |name: &str| markers.iter().any(|m| m.eq_ignore_ascii_case(name));
    if dir.file_name().map(|n| matches(&n.to_string_lossy())).unwrap_or(false) {
        return Some(dir.to_path_buf());
    }
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        if matches(&entry.file_name().to_string_lossy()) && entry.path().is_dir() {
            return Some(dir.to_path_buf());
        }
    }
    None
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
    /// Folder-selection mode (2026-08-14, first user: the host-node
    /// database location). The confirm button picks the CURRENT DIRECTORY
    /// instead of a selected file, so nobody types a path by hand.
    pub dir_mode: bool,
    /// The verb on the confirm button ("Attach", "Open", "Use"); the file
    /// name follows it. The chat attach picker's "Attach" is the default.
    pub pick_verb: String,
    /// Extra quick-access roots shown after the standard ones, as (label,
    /// path): a video screen adds the game's media folder, for example.
    /// Only existing directories are shown.
    pub extra_roots: Vec<(String, PathBuf)>,
    /// Directory names that make the OPEN FOLDER pickable as a whole
    /// (`VIDEO_TS` for a video disc). Empty for an ordinary file picker.
    /// See `folder_offer`.
    pub folder_markers: Vec<String>,
    /// The button that confirms such a folder ("Play disc"). Only drawn
    /// when a marker matches.
    pub folder_pick_label: String,
    /// Several files may be chosen at once (`multiple`, 2026-10-16): each file has a tick, the
    /// ticked ones stay ticked from folder to folder, and the confirm button picks them all.
    pub multiple: bool,
    /// The files ticked so far, in the order they were ticked (only with `multiple`).
    pub chosen: Vec<FsEntry>,
    /// The Details view's sort (widgets/file_views.rs), for this picker while it is open.
    pub sort: super::file_views::SortBy,
    pub sort_desc: bool,
}

impl FilePickerState {
    /// Opens in the folder the last file was picked from, in any picker, while that folder is
    /// still there (operator, 2026-10-16: picking a second file should start "the same place I
    /// just picked a file"); else in the person's home folder. `starting_in` still wins.
    pub fn new(allowed_exts: &[&str], max_size: u64) -> Self {
        let start = memory::with(|m| m.start_folder())
            .or_else(|| {
                std::env::var("USERPROFILE")
                    .or_else(|_| std::env::var("HOME"))
                    .ok()
                    .map(PathBuf::from)
                    .filter(|p| p.is_dir())
            })
            .unwrap_or_else(|| crate::storage::exe_dir());
        Self {
            current_dir: start,
            selected: None,
            allowed_exts: allowed_exts.iter().map(|s| s.to_string()).collect(),
            max_size,
            dir_mode: false,
            pick_verb: "Attach".to_string(),
            extra_roots: Vec::new(),
            folder_markers: Vec::new(),
            folder_pick_label: String::new(),
            multiple: false,
            chosen: Vec::new(),
            sort: Default::default(),
            sort_desc: false,
        }
    }

    /// Start in `dir` when it exists (else keep the default start).
    pub fn starting_in(mut self, dir: Option<PathBuf>) -> Self {
        if let Some(d) = dir.filter(|d| d.is_dir()) {
            self.current_dir = d;
        }
        self
    }

    /// The verb on the confirm button.
    pub fn with_pick_verb(mut self, verb: &str) -> Self {
        self.pick_verb = verb.to_string();
        self
    }

    /// Add a quick-access root (shown only when the directory exists).
    pub fn with_extra_root(mut self, label: &str, dir: PathBuf) -> Self {
        self.extra_roots.push((label.to_string(), dir));
        self
    }

    /// Let the person confirm a WHOLE FOLDER when the one they are looking
    /// at is named `marker` or holds one (a disc's `VIDEO_TS`), under the
    /// button text `label`. Files stay pickable exactly as before.
    pub fn with_folder_marker(mut self, marker: &str, label: &str) -> Self {
        self.folder_markers.push(marker.to_string());
        self.folder_pick_label = label.to_string();
        self
    }

    /// Let the person choose several files at once (operator, 2026-10-16: "Can we have the file
    /// picker allow for picking multiple files instead of one at a time?"). Only for a caller
    /// that takes every one of them.
    pub fn multiple(mut self) -> Self {
        self.multiple = true;
        self
    }

    /// Whether `entry` is ticked.
    fn is_chosen(&self, entry: &FsEntry) -> bool {
        self.chosen.iter().any(|c| c.path == entry.path)
    }

    /// Tick `entry`, or untick it when it is ticked.
    fn toggle(&mut self, entry: &FsEntry) {
        if self.is_chosen(entry) {
            self.chosen.retain(|c| c.path != entry.path);
        } else {
            self.chosen.push(entry.clone());
        }
    }

    /// A picker that chooses a FOLDER (navigate in, confirm the current
    /// directory). Starts from `start` when given, else the user profile.
    pub fn new_dir_picker(start: Option<PathBuf>) -> Self {
        let mut s = Self::new(&[], 0);
        if let Some(p) = start.filter(|p| p.is_dir()) {
            s.current_dir = p;
        }
        s.dir_mode = true;
        s
    }
}

/// What the picker modal reported this frame.
#[derive(Debug, Clone, PartialEq)]
pub enum FilePickerResult {
    /// Still open, nothing chosen yet.
    Open,
    /// User cancelled/closed.
    Cancelled,
    /// User confirmed these: never empty, and exactly one unless the picker is `multiple`.
    Picked(Vec<PathBuf>),
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
    super::thumbnails::next_frame();
    let exts: Vec<&str> = state.allowed_exts.iter().map(|s| s.as_str()).collect();
    let entries = list_dir(&state.current_dir, &exts);

    // Files dropped on the window while the picker is open are its choice (gui/drop_files.rs): the
    // ones its kinds and size allow; a picker for one file takes the first of them.
    if let Some(dropped) = crate::gui::drop_files::take_for_picker(ctx) {
        let fits: Vec<PathBuf> = dropped
            .into_iter()
            .filter(|p| {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let size_ok = state.max_size == 0 || std::fs::metadata(p).map(|m| m.len() <= state.max_size).unwrap_or(false);
                (exts.is_empty() || name_matches_ext(name, &exts)) && size_ok
            })
            .collect();
        if state.dir_mode {
            // A folder picker chooses folders, which are not dropped files.
        } else if fits.is_empty() {
            crate::gui::drop_files::say(ctx, DROPPED_NONE_FIT);
        } else {
            return FilePickerResult::Picked(if state.multiple { fits } else { fits.into_iter().take(1).collect() });
        }
    }

    // A dimmed screen behind the picker, so it reads as a window that is open (operator,
    // 2026-10-17); a click on it is Cancel, as on the app's other dialogs (widgets/dialog.rs).
    let backdrop = egui::Area::new(egui::Id::new(PICKER_ID).with("backdrop"))
        .order(egui::Order::Middle)
        .fixed_pos(egui::pos2(0.0, 0.0))
        .show(ctx, |ui| {
            let screen = ctx.screen_rect();
            ui.painter().rect_filled(screen, 0.0, egui::Color32::from_black_alpha(BACKDROP_ALPHA));
            ui.allocate_rect(screen, egui::Sense::click())
        });
    if backdrop.inner.clicked() {
        return FilePickerResult::Cancelled;
    }

    let window = egui::Window::new(RichText::new(title).color(theme.text_primary()).strong())
        .id(egui::Id::new(PICKER_ID))
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_size(egui::Vec2::new(720.0, 560.0))
        // The frame the app's dialogs wear (widgets/dialog.rs).
        .frame(
            egui::Frame::NONE
                .fill(theme.bg_card())
                .stroke(egui::Stroke::new(1.0, theme.border()))
                .corner_radius(egui::CornerRadius::same(theme.border_radius as u8))
                .inner_margin(theme.card_padding)
                .shadow(egui::epaint::Shadow { offset: [0, 4], blur: 12, spread: 0, color: egui::Color32::from_black_alpha(64) }),
        )
        .show(ctx, |ui| {
            // Places: the person's own folders, the app's, every drive (the DMs' red).
            section(ui, theme, Tint::Red, "Places", |ui| {
                ui.horizontal_wrapped(|ui| {
                    let extra: Vec<(String, PathBuf)> =
                        state.extra_roots.iter().filter(|(_, p)| p.is_dir()).cloned().collect();
                    for (label, path) in quick_roots().into_iter().chain(extra) {
                        if widgets::Button::secondary(&label).show(ui, theme) {
                            state.current_dir = path;
                            state.selected = None;
                        }
                    }
                });
            });
            // The person's own favorite folders, kept on this computer (`file_picker_memory`).
            // One that is not there right now (a drive unplugged) still opens, and the list says
            // why it is empty, so it can be taken out with the same button that put it in.
            let favorites = memory::favorite_labels(&memory::with(|m| m.favorites.clone()));
            if !favorites.is_empty() {
                ui.add_space(theme.spacing_xs);
                section(ui, theme, Tint::Green, "Favorites", |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (label, path) in favorites {
                            let tip = path.display().to_string();
                            if widgets::Button::secondary(&label).tooltip(&tip).show(ui, theme) {
                                state.current_dir = path;
                                state.selected = None;
                            }
                        }
                    });
                });
            }
            ui.add_space(theme.spacing_xs);

            // The open folder and what is in it (the servers' blue).
            section(ui, theme, Tint::Blue, "This folder", |ui| {
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
                let dir = state.current_dir.clone();
                if memory::with(|m| m.is_favorite(&dir)) {
                    if widgets::Button::secondary(REMOVE_FAVORITE)
                        .tooltip("Take this folder out of the Favorites row.")
                        .show(ui, theme)
                    {
                        memory::with(|m| m.remove_favorite(&dir));
                    }
                } else if dir.is_dir()
                    && widgets::Button::secondary(ADD_FAVORITE)
                        .tooltip("Keep this folder in the Favorites row of every file picker on this computer.")
                        .show(ui, theme)
                {
                    memory::with(|m| m.add_favorite(&dir));
                }
            });
            ui.add_space(theme.spacing_xs);

            // Several at once: tick every file here that may be chosen.
            if state.multiple && !state.dir_mode {
                let fitting: Vec<FsEntry> = entries
                    .iter()
                    .filter(|e| !e.is_dir && !(state.max_size > 0 && e.size > state.max_size))
                    .cloned()
                    .collect();
                if !fitting.is_empty() && widgets::Button::secondary(CHOOSE_ALL_HERE).tooltip("Tick every file in this folder.").show(ui, theme) {
                    for e in fitting {
                        if !state.is_chosen(&e) {
                            state.chosen.push(e);
                        }
                    }
                }
                ui.add_space(theme.spacing_xs);
            }

            // How to show the folder (widgets/file_views.rs), remembered on this computer.
            let (mut view, mut across) = memory::with(|m| (m.view, m.across.unwrap_or(4)));
            if super::file_views::toolbar(ui, theme, &mut view, &mut across) {
                memory::with(|m| {
                    m.view = view;
                    m.across = Some(across);
                });
            }
            ui.add_space(theme.spacing_xs);

            // Entry list.
            // As tall as the screen leaves room for, so Choose stays on screen: the Pictures view up to
            // 640 points, a list up to 300.
            let room = ui.ctx().screen_rect().height() - 480.0;
            let list_height = if view == memory::PickerView::Pictures { room.clamp(180.0, 640.0) } else { room.clamp(180.0, 300.0) };
            egui::ScrollArea::vertical()
                .id_salt("file_picker_list")
                .max_height(list_height)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if !state.current_dir.is_dir() {
                        ui.label(
                            RichText::new("This folder is not there right now: its drive may be unplugged, or the folder moved or deleted.")
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                        );
                    } else if entries.is_empty() {
                        ui.label(
                            RichText::new("Nothing here (or nothing matching the allowed types).")
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                        );
                    }
                    let look = super::file_views::Look {
                        selected: state.selected.as_ref().map(|e| e.path.as_path()),
                        chosen: &state.chosen,
                        multiple: state.multiple,
                        dir_mode: state.dir_mode,
                        max_size: state.max_size,
                        notes: None,
                    };
                    let act = match view {
                        memory::PickerView::List => super::file_views::list(ui, theme, &entries, &look),
                        memory::PickerView::Details => super::file_views::details(ui, theme, &entries, &look, &mut state.sort, &mut state.sort_desc),
                        memory::PickerView::Pictures => super::file_views::pictures(ui, theme, &entries, &look, across),
                    };
                    match act {
                        Some(super::file_views::Act::Open(dir)) => {
                            state.current_dir = dir;
                            state.selected = None;
                        }
                        Some(super::file_views::Act::Select(e)) => state.selected = Some(e),
                        Some(super::file_views::Act::Toggle(e)) => state.toggle(&e),
                        Some(super::file_views::Act::Pick(path)) => result = FilePickerResult::Picked(vec![path]),
                        // The picker draws no links beside its files.
                        Some(super::file_views::Act::Note(..)) | None => {}
                    }
                });
            });

            ui.add_space(theme.spacing_xs);
            if state.max_size > 0 {
                ui.label(
                    RichText::new(format!("Max file size: {}", human_size(state.max_size)))
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            }
            if state.multiple && !state.chosen.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(chosen_line(&state.chosen))
                            .size(theme.font_size_small)
                            .color(theme.text_secondary()),
                    );
                    if widgets::Button::secondary(CLEAR_CHOSEN).tooltip("Untick every file.").show(ui, theme) {
                        state.chosen.clear();
                    }
                });
            }
            // The whole-folder offer: a disc's VIDEO_TS is one film spread
            // over numbered files nobody should have to understand, so when
            // the open folder is a disc the picker offers to play THE DISC.
            // Drawn above the ordinary confirm row so it reads as the
            // obvious thing to press when it is there at all.
            if let Some(folder) = folder_offer(&state.current_dir, &state.folder_markers) {
                if widgets::Button::primary(&state.folder_pick_label).show(ui, theme) {
                    result = FilePickerResult::Picked(vec![folder]);
                }
                ui.add_space(theme.spacing_xs);
            }
            ui.horizontal(|ui| {
                // Folder mode confirms the directory currently open; file
                // mode confirms the selected entry.
                let (pick_label, can_pick) = if state.dir_mode {
                    let name = state
                        .current_dir
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("this folder")
                        .to_string();
                    (format!("Use folder: {name}"), true)
                } else if state.multiple {
                    (pick_words(&state.pick_verb, &state.chosen), !state.chosen.is_empty())
                } else {
                    (
                        state
                            .selected
                            .as_ref()
                            .map(|s| format!("{} {}", state.pick_verb, s.name))
                            .unwrap_or_else(|| state.pick_verb.clone()),
                        state.selected.is_some(),
                    )
                };
                // The confirm button in the accent colour: the one thing to press once files are chosen.
                if widgets::Button::primary(&pick_label).disabled(!can_pick).show(ui, theme) {
                    if state.dir_mode {
                        result = FilePickerResult::Picked(vec![state.current_dir.clone()]);
                    } else if state.multiple {
                        result = FilePickerResult::Picked(state.chosen.iter().map(|c| c.path.clone()).collect());
                    } else if let Some(sel) = &state.selected {
                        result = FilePickerResult::Picked(vec![sel.path.clone()]);
                    }
                }
                if widgets::Button::secondary("Cancel").show(ui, theme) {
                    result = FilePickerResult::Cancelled;
                }
            });
        });

    if let Some(w) = window {
        ctx.set_sublayer(backdrop.response.layer_id, w.response.layer_id);
    }

    // A file was picked: the next picker opens in its folder. A folder pick (a folder picker, a
    // disc) leaves the memory alone.
    if let FilePickerResult::Picked(paths) = &result {
        if let Some(path) = paths.last().filter(|p| !state.dir_mode && p.is_file()) {
            memory::with(|m| m.remember_pick(path));
        }
    }
    result
}

/// Said when files dropped on an open picker are all of kinds or sizes it does not take.
pub const DROPPED_NONE_FIT: &str = "None of those files can be chosen here: this asks for other kinds of file, or smaller ones.";

/// The picker window's id (its backdrop is `with("backdrop")`).
pub const PICKER_ID: &str = "file_picker_modal";
/// How dark the screen behind the picker goes: the dialogs' own dimming (widgets/dialog.rs).
const BACKDROP_ALPHA: u8 = 140;
/// One part of the picker in the chat sidebar's colours, darker (widgets/tint_section.rs).
fn section<R>(ui: &mut egui::Ui, theme: &Theme, tint: Tint, title: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    super::tint_section::show(ui, theme, tint, title, add)
}

/// The button that ticks every file in the open folder (a `multiple` picker).
pub const CHOOSE_ALL_HERE: &str = "Choose all here";
/// The button that unticks every file.
pub const CLEAR_CHOSEN: &str = "Clear";

/// The line saying what is ticked: how many, their size, and the first few names.
pub fn chosen_line(chosen: &[FsEntry]) -> String {
    const NAMED: usize = 3;
    let total: u64 = chosen.iter().map(|c| c.size).sum();
    let names: Vec<&str> = chosen.iter().take(NAMED).map(|c| c.name.as_str()).collect();
    let more = chosen.len().saturating_sub(NAMED);
    let count = if chosen.len() == 1 { "1 file".to_string() } else { format!("{} files", chosen.len()) };
    let tail = if more > 0 { format!(", and {more} more") } else { String::new() };
    format!("Chosen: {count}, {}: {}{tail}", human_size(total), names.join(", "))
}

/// The confirm button of a `multiple` picker: the verb, then the one file's name or the count.
pub fn pick_words(verb: &str, chosen: &[FsEntry]) -> String {
    match chosen {
        [] => verb.to_string(),
        [one] => format!("{verb} {}", one.name),
        many => format!("{verb} {} files", many.len()),
    }
}

/// The button beside the path that keeps the open folder in the Favorites row.
pub const ADD_FAVORITE: &str = "Add to favorites";
/// The same button while the open folder is a favorite.
pub const REMOVE_FAVORITE: &str = "Remove from favorites";

#[cfg(test)]
mod tests {
    use super::*;

    /// One headless frame of the picker; `events` are its input. Its output and what it reported.
    fn picker_frame(ctx: &egui::Context, theme: &Theme, state: &mut FilePickerState, events: Vec<egui::Event>) -> (egui::FullOutput, FilePickerResult) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0))),
            events,
            ..Default::default()
        };
        let mut result = FilePickerResult::Open;
        let out = ctx.run(input, |ctx| result = file_picker_modal(ctx, theme, state, "Pick"));
        (out, result)
    }

    /// Where the text exactly `text` is drawn, if it is.
    fn text_at(out: &egui::FullOutput, text: &str) -> Option<egui::Pos2> {
        fn walk(shape: &egui::Shape, text: &str) -> Option<egui::Pos2> {
            match shape {
                egui::Shape::Text(t) if t.galley.text() == text => Some(t.galley.rect.translate(t.pos.to_vec2()).center()),
                egui::Shape::Vec(v) => v.iter().find_map(|s| walk(s, text)),
                _ => None,
            }
        }
        out.shapes.iter().find_map(|cs| walk(&cs.shape, text))
    }

    /// Click the text `text` (after two frames to lay the picker out); what the picker reported.
    fn click_text(ctx: &egui::Context, theme: &Theme, state: &mut FilePickerState, text: &str) -> FilePickerResult {
        picker_frame(ctx, theme, state, vec![]);
        let (out, _) = picker_frame(ctx, theme, state, vec![]);
        let at = text_at(&out, text).unwrap_or_else(|| panic!("{text:?} is drawn"));
        let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::default() };
        picker_frame(ctx, theme, state, vec![egui::Event::PointerMoved(at), button(true)]);
        picker_frame(ctx, theme, state, vec![button(false)]).1
    }

    /// FAVORITES, AND THE NEXT PICKER OPENS WHERE THE LAST FILE WAS PICKED (operator, 2026-10-16:
    /// "I want to be able to add/remove favorites, like the E:/models/ folder. When I pick a file and
    /// go to pick another file the path it should have loaded ... be the same place I just picked a
    /// file"). Through the picker itself: Add to favorites puts the open folder in the Favorites row,
    /// its button opens it from anywhere, a picked file sends the next picker to its folder, and
    /// Remove from favorites takes it out again.
    ///
    /// Seen red 2026-10-16: with `remember_pick` left out, "a new picker opens in the folder the file
    /// was picked from" found the home folder; with the Favorites row left out, "\"models_fav_test\" is
    /// drawn" found no such button.
    #[test]
    fn favorites_and_the_next_picker_opens_where_the_last_file_was_picked() {
        let ctx = egui::Context::default();
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        let root = tmp("favorites");
        let models = root.join("models_fav_test");
        std::fs::create_dir_all(&models).unwrap();
        std::fs::write(models.join("part.blend"), "x").unwrap();

        let mut picker = FilePickerState::new(&[], 0).with_pick_verb("Add");
        assert!(!same(&picker.current_dir, &models), "a first picker does not start in the test folder");
        picker.current_dir = models.clone();
        click_text(&ctx, &theme, &mut picker, ADD_FAVORITE);
        assert!(memory::with(|m| m.is_favorite(&models)), "Add to favorites keeps the folder");
        let (out, _) = picker_frame(&ctx, &theme, &mut picker, vec![]);
        assert!(text_at(&out, REMOVE_FAVORITE).is_some(), "the button now offers to remove it");

        picker.current_dir = root.to_path_buf();
        click_text(&ctx, &theme, &mut picker, "models_fav_test");
        assert!(same(&picker.current_dir, &models), "its button is in the Favorites row, and opens it");

        // Pick the file there.
        click_text(&ctx, &theme, &mut picker, "part.blend  (1 B)");
        let picked = click_text(&ctx, &theme, &mut picker, "Add part.blend");
        assert_eq!(picked, FilePickerResult::Picked(vec![models.join("part.blend")]), "the file is picked");
        let next = FilePickerState::new(&[], 0);
        assert!(same(&next.current_dir, &models), "a new picker opens in the folder the file was picked from: {:?}", next.current_dir);

        let mut picker = next;
        click_text(&ctx, &theme, &mut picker, REMOVE_FAVORITE);
        assert!(!memory::with(|m| m.is_favorite(&models)), "Remove from favorites takes it out");
        let (out, _) = picker_frame(&ctx, &theme, &mut picker, vec![]);
        assert!(text_at(&out, "Favorites").is_none(), "and with none left there is no Favorites row");

        // A favorite that is not there right now still opens, and says why it is empty.
        let gone = root.join("unplugged");
        memory::with(|m| m.add_favorite(&gone));
        picker.current_dir = gone.clone();
        let (out, _) = picker_frame(&ctx, &theme, &mut picker, vec![]);
        assert!(
            text_at(&out, "This folder is not there right now: its drive may be unplugged, or the folder moved or deleted.").is_some(),
            "a missing folder says so"
        );
        click_text(&ctx, &theme, &mut picker, REMOVE_FAVORITE);
        assert!(!memory::with(|m| m.is_favorite(&gone)), "and can be taken out the same way");
    }

    fn same(a: &Path, b: &Path) -> bool {
        memory::same_folder(a, b)
    }

    fn tmp(name: &str) -> crate::test_temp::TempPath {
        crate::test_temp::dir(&format!("fb_test_{name}"))
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

    /// The whole-folder offer, which is how a video disc is picked: it
    /// stands on the drive that holds a VIDEO_TS and on the folder itself,
    /// and nowhere else. Proven able to fail: returning `Some` for any
    /// directory makes the "an ordinary folder" assertion fire.
    #[test]
    fn a_folder_marker_offers_the_disc_from_the_drive_or_the_folder() {
        let root = tmp("marker");
        let drive = root.join("disc_drive");
        let video_ts = drive.join("VIDEO_TS");
        std::fs::create_dir_all(&video_ts).unwrap();
        let markers = vec!["VIDEO_TS".to_string()];
        assert_eq!(folder_offer(&drive, &markers), Some(drive.clone()), "the drive holding it");
        assert_eq!(folder_offer(&video_ts, &markers), Some(video_ts.clone()), "the folder itself");
        // Case does not matter: a disc written elsewhere may be lower case.
        assert_eq!(folder_offer(&video_ts, &["video_ts".to_string()]), Some(video_ts.clone()));

        let plain = root.join("holiday_photos");
        std::fs::create_dir_all(&plain).unwrap();
        assert_eq!(folder_offer(&plain, &markers), None, "an ordinary folder offers nothing");
        assert_eq!(folder_offer(&drive, &[]), None, "a picker with no markers never offers a folder");
        assert_eq!(folder_offer(&root.join("nowhere"), &markers), None, "a path that is not there");
        // A FILE named like the marker is not a disc.
        let faker = root.join("faker");
        std::fs::create_dir_all(&faker).unwrap();
        std::fs::write(faker.join("VIDEO_TS"), "not a folder").unwrap();
        assert_eq!(folder_offer(&faker, &markers), None);
    }

    /// The quick row never offers a path that is not there, drives
    /// included, and asking twice inside the cache window gives the same
    /// answer without asking the machine again.
    ///
    /// The row must also be able to REACH a whole drive, which is the thing
    /// it could not do until 2026-09-18: on a machine with more than one
    /// drive letter there is no other way in, because this picker has
    /// nowhere to type a path. A single-drive machine (and any machine that
    /// is not Windows) legitimately offers none, so the assertion is on the
    /// shape of what a drive button is, not on there being one.
    #[test]
    fn quick_roots_are_all_real_directories() {
        for (label, path) in quick_roots() {
            assert!(path.is_dir(), "{label} -> {}", path.display());
        }
        for (label, path) in drive_roots_cached() {
            assert!(path.is_dir(), "a drive button must point somewhere real: {label}");
            assert!(label.contains('('), "a drive button names its letter in brackets: {label}");
            assert!(
                quick_roots().iter().any(|(l, p)| l == &label && p == &path),
                "a drive the machine has must appear in the row: {label}"
            );
        }
        assert_eq!(drive_roots_cached(), drive_roots_cached(), "the cached answer is stable");
    }

    #[test]
    fn ext_matching_is_case_insensitive_and_dot_safe() {
        assert!(name_matches_ext("Photo.PNG", &["png"]));
        assert!(name_matches_ext("a.tar.gz", &["tar.gz"]));
        assert!(!name_matches_ext("notpng", &["png"]));
        assert!(!name_matches_ext("file.png.exe", &["png"]));
    }

    /// SEVERAL FILES AT ONCE (operator, 2026-10-16: "Can we have the file picker allow for picking
    /// multiple files instead of one at a time?"). In a `multiple` picker each file has a tick:
    /// Choose all here ticks every file that fits (never one over the size limit), a click unticks,
    /// ticks stay from folder to folder, the line says what is ticked, and the confirm button picks
    /// them all in the order they were ticked; Clear unticks everything.
    ///
    /// Seen red 2026-10-16: with Choose all here not leaving out an oversized file, "ticks every file
    /// that fits" listed huge.stl too; with the confirm button picking only the first ticked, "both,
    /// in the order ticked" got one.
    #[test]
    fn several_files_are_chosen_across_folders_and_picked_together() {
        let ctx = egui::Context::default();
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        let root = tmp("multiple");
        let parts = root.join("parts");
        let pictures = root.join("pictures");
        std::fs::create_dir_all(&parts).unwrap();
        std::fs::create_dir_all(&pictures).unwrap();
        std::fs::write(parts.join("one.stl"), "abc").unwrap();
        std::fs::write(parts.join("two.stl"), "abcd").unwrap();
        std::fs::write(parts.join("huge.stl"), "x".repeat(20)).unwrap();
        std::fs::write(pictures.join("cover.png"), "xy").unwrap();
        let names = |p: &FilePickerState| p.chosen.iter().map(|c| c.name.clone()).collect::<Vec<_>>();

        let mut picker = FilePickerState::new(&[], 10).with_pick_verb("Add").multiple();
        picker.current_dir = parts.clone();
        click_text(&ctx, &theme, &mut picker, CHOOSE_ALL_HERE);
        assert_eq!(names(&picker), ["one.stl", "two.stl"], "Choose all here ticks every file that fits");
        click_text(&ctx, &theme, &mut picker, "one.stl  (3 B)");
        assert_eq!(names(&picker), ["two.stl"], "a click unticks a ticked file");
        picker.current_dir = pictures.clone();
        click_text(&ctx, &theme, &mut picker, "cover.png  (2 B)");
        assert_eq!(names(&picker), ["two.stl", "cover.png"], "ticks stay from folder to folder");
        let (out, _) = picker_frame(&ctx, &theme, &mut picker, vec![]);
        assert!(text_at(&out, "Chosen: 2 files, 6 B: two.stl, cover.png").is_some(), "the line says what is ticked");
        let picked = click_text(&ctx, &theme, &mut picker, "Add 2 files");
        assert_eq!(picked, FilePickerResult::Picked(vec![parts.join("two.stl"), pictures.join("cover.png")]), "both, in the order ticked");
        assert!(same(&FilePickerState::new(&[], 0).current_dir, &pictures), "the next picker opens where the last ticked file was");

        let mut picker = FilePickerState::new(&[], 10).with_pick_verb("Add").multiple();
        picker.current_dir = parts.clone();
        click_text(&ctx, &theme, &mut picker, CHOOSE_ALL_HERE);
        click_text(&ctx, &theme, &mut picker, CLEAR_CHOSEN);
        assert!(picker.chosen.is_empty(), "Clear unticks everything");
        let (out, _) = picker_frame(&ctx, &theme, &mut picker, vec![]);
        assert!(text_at(&out, "Add").is_some() && text_at(&out, CLEAR_CHOSEN).is_none(), "nothing to pick and nothing to clear");

        // A one-file picker is unchanged: no ticks, a click selects, and it picks exactly one.
        let mut single = FilePickerState::new(&[], 10).with_pick_verb("Add");
        single.current_dir = parts.clone();
        let (out, _) = picker_frame(&ctx, &theme, &mut single, vec![]);
        assert!(text_at(&out, CHOOSE_ALL_HERE).is_none(), "a one-file picker has no Choose all here");
        click_text(&ctx, &theme, &mut single, "one.stl  (3 B)");
        assert_eq!(click_text(&ctx, &theme, &mut single, "Add one.stl"), FilePickerResult::Picked(vec![parts.join("one.stl")]));
    }


    /// THE PICKER OPENS OVER A DIMMED SCREEN (operator, 2026-10-17: "a faded background behind the
    /// popup window to indicate that there is a window open"): the window is above its backdrop, and
    /// a click on the dimmed screen is Cancel, as on the app's other dialogs.
    ///
    /// Seen red 2026-10-17 with the backdrop's click ignored: "a click on the dimmed screen cancels"
    /// got Open.
    #[test]
    fn the_picker_opens_over_a_dimmed_screen_that_cancels_it() {
        let ctx = egui::Context::default();
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        let mut picker = FilePickerState::new(&[], 0);
        picker.current_dir = tmp("backdrop").to_path_buf();
        picker_frame(&ctx, &theme, &mut picker, vec![]);
        picker_frame(&ctx, &theme, &mut picker, vec![]);
        let window = ctx.memory(|m| m.area_rect(egui::Id::new(PICKER_ID))).expect("drawn");
        let top = ctx.layer_id_at(window.center()).map(|l| l.id);
        assert_eq!(top, Some(egui::Id::new(PICKER_ID)), "the window is above its backdrop");
        assert_eq!(ctx.layer_id_at(egui::pos2(5.0, 5.0)).map(|l| l.id), Some(egui::Id::new(PICKER_ID).with("backdrop")), "the screen around it is the backdrop");
        let at = egui::pos2(5.0, 5.0);
        let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::default() };
        picker_frame(&ctx, &theme, &mut picker, vec![egui::Event::PointerMoved(at), button(true)]);
        let (_, result) = picker_frame(&ctx, &theme, &mut picker, vec![button(false)]);
        assert_eq!(result, FilePickerResult::Cancelled, "a click on the dimmed screen cancels");
    }
}
