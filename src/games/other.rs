//! Games from anywhere else: a game installed from its maker's own website (Dwarf Fortress Classic
//! from Bay 12 Games, say), or any program the person wants on the list. The person adds it by its
//! program with "Add a game"; it is kept in the app's settings (config.json, `added_games`) on this
//! computer only, and Remove takes it off the list without touching the game. No store is involved:
//! Play starts its own program in its own folder, which is what its desktop shortcut does (the
//! finding, section 5).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{on_disk, Game, Launch, State, Store};

/// A game the person added: the name it shows (its program's file name until renamed) and its
/// program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddedGame {
    pub name: String,
    pub exe: PathBuf,
}

/// The name an added game starts with: its program's file name without `.exe`.
pub fn name_from_file(exe: &Path) -> String {
    exe.file_stem().map(|s| s.to_string_lossy().trim().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| exe.display().to_string())
}

/// The added games as the page shows them, in the order added. Reads whether each program and its
/// folder are there (two checks per game, done when the list changes and when the page reads the
/// stores, never every frame).
pub fn games(added: &[AddedGame]) -> Vec<Game> {
    added
        .iter()
        .map(|a| {
            let folder = a.exe.parent().map(Path::to_path_buf).unwrap_or_default();
            let state = on_disk(&folder, Some(&a.exe), State::Ready);
            let play = (state == State::Ready).then(|| Launch::Program { exe: a.exe.clone(), args: String::new(), dir: folder.clone() });
            Game { store: Store::Added, id: a.exe.display().to_string(), name: a.name.clone(), show: a.exe.clone(), folder_here: folder.is_dir(), folder, state, play }
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
            AddedGame { name: "Dwarf Fortress Classic".into(), exe: exe.clone() },
            AddedGame { name: "Gone".into(), exe: dir.join("gone.exe") },
            AddedGame { name: "Unplugged".into(), exe: dir.join("no such folder").join("x.exe") },
        ];
        let games = games(&added);
        assert_eq!(games[0].play, Some(Launch::Program { exe: exe.clone(), args: String::new(), dir: dir.to_path_buf() }));
        assert_eq!(games[0].show, exe, "Show in folder shows its program");
        assert_eq!(games[1].state, State::ProgramGone, "a program gone from its folder says so");
        assert_eq!(games[2].state, State::FolderGone, "a folder gone says so");
        assert!(games[1].play.is_none() && games[2].play.is_none());
    }
}
