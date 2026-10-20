//! What the in-app file picker remembers on this computer (2026-10-16): the person's favorite
//! folders, shown as their own row in every picker, and the folder a file was last picked from,
//! where the next picker opens. And (2026-10-17) the chat channels the person favourites for Add to
//! Media's "Also post in chat", listed first after the one open in the chat.
//!
//! The operator, on the Media Edit window: "I want to be able to add/remove favorites, like the
//! E:/models/ folder. When I pick a file and go to pick another file the path it should have
//! loaded when picking a file the second time be the same place I just picked a file."
//!
//! Kept in `file_picker.json` beside `config.json`, so it follows the install (portable mode
//! included) and never leaves this computer: a list of someone's folders is nobody else's
//! business, so it is not part of settings sync. Nothing is written while the storage choice is
//! still undecided on a first boot; the pickers remember for that run only.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// At most this many favorites are kept: the row wraps, but past this it stops being a shortcut.
pub const MAX_FAVORITES: usize = 100;

/// The file the memory is kept in, beside `config.json`.
pub const FILE_NAME: &str = "file_picker.json";

/// What the pickers remember.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PickerMemory {
    /// Favorite folders, in the order they were added.
    #[serde(default)]
    pub favorites: Vec<PathBuf>,
    /// The folder the last file was picked from, where the next picker opens.
    #[serde(default)]
    pub last_folder: Option<PathBuf>,
    /// Favourite chat channels to post uploads in, each as `<server>\n<channel id>`
    /// (files/add.rs `post_key`).
    #[serde(default)]
    pub post_favorites: Vec<String>,
    /// How the pickers show a folder (2026-10-17): a list, a table, or pictures `across` to a row.
    #[serde(default)]
    pub view: PickerView,
    /// Pictures to a row in the Pictures view, 1 to 8 (None: 4).
    #[serde(default)]
    pub across: Option<u8>,
}

/// How the file pickers show a folder (widgets/file_views.rs).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PickerView {
    /// One row each: its type, its name, its size.
    #[default]
    List,
    /// A table: type, name, size and date changed, sorted by a click on a heading.
    Details,
    /// A grid of previews, a picture's own and any other file's type.
    Pictures,
}

/// One folder spelled the way it is compared: no trailing separator (a drive root keeps its own),
/// and on Windows in lower case, since `E:\Models` and `e:\models\` are the same folder there.
fn key(dir: &Path) -> String {
    let s = dir.to_string_lossy();
    let trimmed = s.trim_end_matches(['\\', '/']);
    // "E:\" trims to "E:", which on Windows names the drive's CURRENT folder, not its root;
    // compared, not opened, so that is fine, and both spellings of the root agree.
    let trimmed = if trimmed.is_empty() { s.as_ref() } else { trimmed };
    if cfg!(windows) {
        trimmed.to_lowercase()
    } else {
        trimmed.to_string()
    }
}

/// Whether two paths name the same folder (`key`).
pub fn same_folder(a: &Path, b: &Path) -> bool {
    key(a) == key(b)
}

impl PickerMemory {
    pub fn is_favorite(&self, dir: &Path) -> bool {
        self.favorites.iter().any(|f| same_folder(f, dir))
    }

    /// Whether channel `key` (files/add.rs `post_key`) is a favourite to post uploads in.
    pub fn is_post_favorite(&self, key: &str) -> bool {
        self.post_favorites.iter().any(|k| k == key)
    }

    /// Make channel `key` a favourite, or stop it being one.
    pub fn toggle_post_favorite(&mut self, key: &str) {
        if let Some(i) = self.post_favorites.iter().position(|k| k == key) {
            self.post_favorites.remove(i);
        } else if self.post_favorites.len() < MAX_FAVORITES {
            self.post_favorites.push(key.to_string());
        }
    }

    /// Add `dir` to the favorites (at the end). False when it was one already, or the list is full.
    pub fn add_favorite(&mut self, dir: &Path) -> bool {
        if self.is_favorite(dir) || self.favorites.len() >= MAX_FAVORITES {
            return false;
        }
        self.favorites.push(dir.to_path_buf());
        true
    }

    /// Take `dir` out of the favorites. False when it was not one.
    pub fn remove_favorite(&mut self, dir: &Path) -> bool {
        let before = self.favorites.len();
        self.favorites.retain(|f| !same_folder(f, dir));
        self.favorites.len() != before
    }

    /// A file was picked: the next picker opens in its folder.
    pub fn remember_pick(&mut self, file: &Path) {
        if let Some(folder) = file.parent().filter(|p| !p.as_os_str().is_empty()) {
            self.last_folder = Some(folder.to_path_buf());
        }
    }

    /// Where a new picker opens: the last file's folder while it is still there.
    pub fn start_folder(&self) -> Option<PathBuf> {
        self.last_folder.clone().filter(|d| d.is_dir())
    }
}

/// Each favorite with the name its button shows: the folder's own name, or the whole path when
/// another favorite has the same name (two `models` folders on two drives) or it is a drive root.
pub fn favorite_labels(favorites: &[PathBuf]) -> Vec<(String, PathBuf)> {
    let name = |p: &Path| p.file_name().map(|n| n.to_string_lossy().to_string());
    favorites
        .iter()
        .map(|p| {
            let label = match name(p) {
                Some(n) if favorites.iter().filter(|o| name(o).map(|m| m.eq_ignore_ascii_case(&n)).unwrap_or(false)).count() == 1 => n,
                _ => p.display().to_string(),
            };
            (label, p.clone())
        })
        .collect()
}

/// Read the memory from `path`; nothing there, or something unreadable, is an empty memory.
pub fn load(path: &Path) -> PickerMemory {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Write the memory to `path`, through a temporary file so a crash never leaves half of one.
pub fn save(path: &Path, memory: &PickerMemory) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(memory).map_err(std::io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

/// Where the memory is kept: beside `config.json`, or nowhere while the storage choice is still
/// undecided (nothing may be written yet).
#[cfg(not(test))]
fn store_path() -> Option<PathBuf> {
    if crate::storage::mode() == crate::storage::StorageMode::Undecided {
        return None;
    }
    Some(crate::config::AppConfig::config_path().with_file_name(FILE_NAME))
}

#[cfg(not(test))]
static MEMORY: std::sync::Mutex<Option<PickerMemory>> = std::sync::Mutex::new(None);

// Tests keep their own memory per test thread, so no test reads another's favorites or writes
// the real file.
#[cfg(test)]
thread_local! {
    static MEMORY: std::cell::RefCell<PickerMemory> = std::cell::RefCell::new(PickerMemory::default());
}

/// Read or change what the pickers remember. A change is saved at once.
pub fn with<R>(f: impl FnOnce(&mut PickerMemory) -> R) -> R {
    #[cfg(not(test))]
    {
        let mut guard = MEMORY.lock().unwrap_or_else(|e| e.into_inner());
        let path = store_path();
        let memory = guard.get_or_insert_with(|| path.as_deref().map(load).unwrap_or_default());
        let before = memory.clone();
        let answer = f(memory);
        if *memory != before {
            if let Some(path) = path {
                if let Err(e) = save(&path, memory) {
                    log::warn!("file picker: could not save {}: {e}", path.display());
                }
            }
        }
        answer
    }
    #[cfg(test)]
    {
        MEMORY.with(|m| f(&mut m.borrow_mut()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn favorites_are_added_once_and_removed_by_any_spelling_of_the_folder() {
        let mut m = PickerMemory::default();
        let models = PathBuf::from(if cfg!(windows) { r"E:\models" } else { "/media/models" });
        assert!(m.add_favorite(&models), "added");
        assert!(!m.add_favorite(&models), "not twice");
        let other_spelling = PathBuf::from(if cfg!(windows) { r"e:\Models\" } else { "/media/models/" });
        assert!(m.is_favorite(&other_spelling), "the same folder spelled another way is the favorite");
        assert!(!m.add_favorite(&other_spelling), "and is not added again");
        assert!(m.remove_favorite(&other_spelling), "removed by it");
        assert!(m.favorites.is_empty() && !m.remove_favorite(&models), "gone, and removing again does nothing");
        for i in 0..MAX_FAVORITES + 5 {
            m.add_favorite(&PathBuf::from(format!("folder{i}")));
        }
        assert_eq!(m.favorites.len(), MAX_FAVORITES, "the list stops at its limit");
    }

    #[test]
    fn the_next_picker_opens_where_the_last_file_was_picked_while_that_folder_is_there() {
        let dir = crate::test_temp::dir("fpm_start");
        let mut m = PickerMemory::default();
        assert_eq!(m.start_folder(), None, "nothing picked yet");
        m.remember_pick(&dir.join("part.blend"));
        assert_eq!(m.start_folder(), Some(dir.to_path_buf()), "the picked file's folder");
        m.remember_pick(&dir.join("gone").join("part.blend"));
        assert_eq!(m.start_folder(), None, "a folder that is not there is not opened");
    }

    #[test]
    fn a_favorite_shows_its_folder_name_unless_another_shares_it() {
        let root = if cfg!(windows) { r"E:\" } else { "/" };
        let favs = vec![
            PathBuf::from(root).join("models"),
            PathBuf::from(root).join("work").join("Models"),
            PathBuf::from(root).join("photos"),
        ];
        let labels: Vec<String> = favorite_labels(&favs).into_iter().map(|(l, _)| l).collect();
        assert_eq!(labels[2], "photos");
        assert_eq!(labels[0], favs[0].display().to_string(), "two folders named models show their paths");
        assert_eq!(labels[1], favs[1].display().to_string());
        let drive = favorite_labels(&[PathBuf::from(root)]);
        assert_eq!(drive[0].0, PathBuf::from(root).display().to_string(), "a drive root shows its path");
    }

    #[test]
    fn the_memory_is_saved_and_read_back_and_a_damaged_file_is_an_empty_memory() {
        let dir = crate::test_temp::dir("fpm_save");
        let path = dir.join(FILE_NAME);
        assert_eq!(load(&path), PickerMemory::default(), "no file yet");
        let m = PickerMemory { favorites: vec![dir.join("models")], last_folder: Some(dir.join("parts")), post_favorites: vec!["https://h\ngeneral".into()], view: PickerView::Pictures, across: Some(2) };
        save(&path, &m).expect("saved");
        assert_eq!(load(&path), m, "read back");
        assert!(!path.with_extension("json.tmp").exists(), "no temporary file left");
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(load(&path), PickerMemory::default(), "a damaged file reads as empty");
    }
}
