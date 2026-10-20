//! What the person added themselves, the Apps page's "Added by you" (2026-10-19): a game installed
//! from its maker's own website (Dwarf Fortress Classic from Bay 12 Games, say), or any other
//! program the person wants on the list. The person adds it by its program with "Add a program";
//! it is kept in the app's settings (config.json, `apps_added`) on this computer only, and Remove
//! takes it off the list without touching the program. No store is involved: Launch starts its own
//! program in its own folder, which is what its desktop shortcut does (the 2026-10-13 finding,
//! section 5). Each is listed as a `Game` from `Store::Added`, so it starts through the one
//! launcher like the rest.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{on_disk, Game, Launch, State, Store};

/// A game or program the person added: the name it shows (its program's file name until renamed) and its
/// program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddedApp {
    pub name: String,
    pub exe: PathBuf,
}

/// The name a thing added starts with: its program's file name without `.exe`.
pub fn name_from_file(exe: &Path) -> String {
    exe.file_stem().map(|s| s.to_string_lossy().trim().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| exe.display().to_string())
}

/// What was added, as the page shows it, in the order added. Reads whether each program and its
/// folder are there (two checks each, done when the list changes and when the page reads the
/// stores, never every frame).
pub fn games(added: &[AddedApp]) -> Vec<Game> {
    added
        .iter()
        .map(|a| {
            let folder = a.exe.parent().map(Path::to_path_buf).unwrap_or_default();
            let state = on_disk(&folder, Some(&a.exe), State::Ready);
            let play = (state == State::Ready).then(|| Launch::Program { exe: a.exe.clone(), args: String::new(), dir: folder.clone() });
            // No details: an added game's folder may hold anything (its size is not added up),
            // and no store keeps a record of it.
            Game { store: Store::Added, id: a.exe.display().to_string(), name: a.name.clone(), show: a.exe.clone(), folder_here: folder.is_dir(), folder, state, play, details: Default::default() }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AN ADDED GAME is named by its program's file name, plays its own program in its own folder,
    /// and says so when its program (or its whole folder) is gone.
    ///
    /// Seen red 2026-10-18 with `games` leaving out the program check (`on_disk(&folder, None, ..)`):
    /// "a program gone from its folder says so" (left: Ready, right: ProgramGone).
    #[test]
    fn an_added_game() {
        let dir = crate::test_temp::dir("added_game");
        let exe = dir.join("Dwarf Fortress.exe");
        std::fs::write(&exe, "x").unwrap();
        assert_eq!(name_from_file(&exe), "Dwarf Fortress");
        let added = vec![
            AddedApp { name: "Dwarf Fortress Classic".into(), exe: exe.clone() },
            AddedApp { name: "Gone".into(), exe: dir.join("gone.exe") },
            AddedApp { name: "Unplugged".into(), exe: dir.join("no such folder").join("x.exe") },
        ];
        let games = games(&added);
        assert_eq!(games[0].play, Some(Launch::Program { exe: exe.clone(), args: String::new(), dir: dir.to_path_buf() }));
        assert_eq!(games[0].show, exe, "Show in folder shows its program");
        assert_eq!(games[1].state, State::ProgramGone, "a program gone from its folder says so");
        assert_eq!(games[2].state, State::FolderGone, "a folder gone says so");
        assert!(games[1].play.is_none() && games[2].play.is_none());
    }
}
