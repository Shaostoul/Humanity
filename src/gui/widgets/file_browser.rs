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
            dir_mode: false,
            pick_verb: "Attach".to_string(),
            extra_roots: Vec::new(),
            folder_markers: Vec::new(),
            folder_pick_label: String::new(),
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
                let extra: Vec<(String, PathBuf)> =
                    state.extra_roots.iter().filter(|(_, p)| p.is_dir()).cloned().collect();
                for (label, path) in quick_roots().into_iter().chain(extra) {
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
            // The whole-folder offer: a disc's VIDEO_TS is one film spread
            // over numbered files nobody should have to understand, so when
            // the open folder is a disc the picker offers to play THE DISC.
            // Drawn above the ordinary confirm row so it reads as the
            // obvious thing to press when it is there at all.
            if let Some(folder) = folder_offer(&state.current_dir, &state.folder_markers) {
                if widgets::Button::primary(&state.folder_pick_label).show(ui, theme) {
                    result = FilePickerResult::Picked(folder);
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
                if ui
                    .add_enabled(
                        can_pick,
                        egui::Button::new(
                            RichText::new(pick_label).color(theme.text_primary()),
                        ),
                    )
                    .clicked()
                {
                    if state.dir_mode {
                        result = FilePickerResult::Picked(state.current_dir.clone());
                    } else if let Some(sel) = &state.selected {
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
}
