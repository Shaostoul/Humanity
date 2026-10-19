//! GOG's installed games, read from GOG's own records and never written to.
//!
//! GOG's installers (and GOG GALAXY) record each game under the registry key
//! `HKLM\SOFTWARE\WOW6432Node\GOG.com\Games\<game id>`, whose `path` value is the game's folder;
//! that is read with KEY_READ only. Beside the game's files is `goggame-<game id>.info`, JSON whose
//! `playTasks` list how the game is started; the one marked `"isPrimary": true` is the game itself
//! (GOG's developer documentation describes these tasks: the finding, 3.3).
//!
//! Play starts that task's own program, with its arguments, in the working directory it names (or
//! the game's folder when it names none), exactly as the desktop shortcut GOG's installer made
//! does. GOG says its games need no launcher: "No, you don't need any additional apps or
//! launchers" (the finding, 3.2). GOG GALAXY's command line is not used: GOG does not document it.
//! There is no GOG button either, for the same reason: GOG documents no link that opens GOG GALAXY.

use std::path::{Path, PathBuf};

use super::{on_disk, read_small, sort_by_name, Details, Game, Launch, State, Store};

/// One game GOG's registry key names: its id, its folder, and the name the key gives it (used
/// when the game's `.info` does not give one).
#[derive(Debug, Clone, PartialEq)]
pub struct Installed {
    pub id: String,
    pub folder: PathBuf,
    pub name: String,
}

/// How a game is started: its program, its arguments (as one string, passed as written) and the
/// folder it starts in.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayTask {
    pub exe: PathBuf,
    pub args: String,
    pub dir: PathBuf,
}

/// The primary play task in a `goggame-<id>.info` text, resolved against the game's `folder`: the
/// task marked primary that names a program, else the first "game" task that does. `None` when
/// there is none (a GOG game started some other way is listed without Play).
pub fn primary_task(info: &serde_json::Value, folder: &Path) -> Option<PlayTask> {
    let tasks = info.get("playTasks")?.as_array()?;
    let is_file = |t: &&serde_json::Value| {
        let kind = t.get("type").and_then(|v| v.as_str()).unwrap_or("FileTask");
        kind == "FileTask" && t.get("path").and_then(|v| v.as_str()).is_some_and(|p| !p.trim().is_empty())
    };
    let primary = tasks.iter().filter(is_file).find(|t| t.get("isPrimary").and_then(|v| v.as_bool()) == Some(true));
    let task = primary.or_else(|| tasks.iter().filter(is_file).find(|t| t.get("category").and_then(|v| v.as_str()) == Some("game")))?;
    let text = |key: &str| task.get(key).and_then(|v| v.as_str()).map(str::trim).unwrap_or("").to_string();
    let exe = folder.join(text("path"));
    let working = text("workingDir");
    let dir = if working.is_empty() { folder.to_path_buf() } else { folder.join(working) };
    Some(PlayTask { exe, args: text("arguments"), dir })
}

/// Every game GOG's key names, sorted by name.
pub fn read(installed: &[Installed]) -> Vec<Game> {
    let mut out: Vec<Game> = Vec::new();
    for entry in installed {
        // The id is part of a file name, so only an id of digits is used.
        if entry.id.is_empty() || !entry.id.chars().all(|c| c.is_ascii_digit()) || out.iter().any(|g| g.id == entry.id) {
            continue;
        }
        out.push(read_game(entry));
    }
    sort_by_name(&mut out);
    out
}

fn read_game(entry: &Installed) -> Game {
    let folder = entry.folder.clone();
    let info: Option<serde_json::Value> = read_small(&folder.join(format!("goggame-{}.info", entry.id))).and_then(|t| serde_json::from_str(&t).ok());
    let task = info.as_ref().and_then(|i| primary_task(i, &folder));
    let name = info.as_ref().and_then(|i| i.get("name")).and_then(|v| v.as_str()).map(str::trim).filter(|n| !n.is_empty()).map(str::to_string).unwrap_or_else(|| {
        if entry.name.trim().is_empty() {
            format!("GOG game {}", entry.id)
        } else {
            entry.name.trim().to_string()
        }
    });
    let state = match &task {
        Some(t) => on_disk(&folder, Some(&t.exe), State::Ready),
        None if !folder.is_dir() => State::FolderGone,
        // No file saying how it starts: listed, without Play.
        None => State::ProgramGone,
    };
    let play = match (&task, state) {
        (Some(t), State::Ready) => Some(Launch::Program { exe: t.exe.clone(), args: t.args.clone(), dir: t.dir.clone() }),
        _ => None,
    };
    // Its details (2026-10-19): GOG records no size, so its folder is added up here, on the
    // page's worker thread (none past `folder_size`'s limit, rather than a number cut short).
    let details = Details { size: super::folder_size(&folder), art: super::art::program(Store::Gog, task.as_ref().map(|t| t.exe.as_path())), ..Details::default() };
    Game { store: Store::Gog, id: entry.id.clone(), name, show: folder.clone(), folder_here: folder.is_dir(), folder, state, play, details }
}

/// GOG's installed games on this computer, from its registry key (read only); `None` when the key
/// is not there (GOG has never installed anything here).
#[cfg(windows)]
pub fn installed_on_this_computer() -> Option<Vec<Installed>> {
    use super::registry::{open, Root};
    let games = open(Root::LocalMachine, r"SOFTWARE\WOW6432Node\GOG.com\Games").or_else(|| open(Root::LocalMachine, r"SOFTWARE\GOG.com\Games"))?;
    let mut out = Vec::new();
    for id in games.subkeys() {
        let Some(key) = games.open(&id) else { continue };
        let Some(path) = key.string("path").map(|p| p.trim().to_string()).filter(|p| !p.is_empty()) else { continue };
        let name = key.string("gameName").unwrap_or_default();
        out.push(Installed { id, folder: PathBuf::from(path), name });
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::fixtures;

    /// THE PRIMARY PLAY TASK: Old Forge's .info lists its manual first and the game, marked
    /// primary, second; Play is the game's own program with its arguments as written and the
    /// working directory the .info names. Gone Valley, whose folder is not there, has no Play.
    ///
    /// Seen red 2026-10-18 with the primary task picked as the first task with a path (the manual,
    /// manual.pdf, which is not on the disk): Old Forge's state, left: ProgramGone, right: Ready.
    #[test]
    fn the_primary_play_task() {
        let fx = fixtures::gog("gog_primary");
        let games = read(&fx.installed);
        let names: Vec<&str> = games.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, vec!["Gone Valley", "Old Forge"]);
        let forge = &games[1];
        assert_eq!(forge.state, State::Ready);
        assert_eq!(
            forge.play,
            Some(Launch::Program { exe: fx.forge.join("bin\\forge.exe"), args: "-windowed -lang en".into(), dir: fx.forge.join("bin") }),
            "Play is the primary task's program"
        );
        assert!(forge.store_button().is_none(), "GOG documents no link to open GOG GALAXY, so there is no button");
        let gone = &games[0];
        assert_eq!(gone.state, State::FolderGone, "its folder is not there");
        assert_eq!(gone.play, None);
    }

    /// A GOG GAME'S SIZE IS ITS FOLDER ADDED UP (2026-10-19): GOG records none, so the reader adds
    /// up Old Forge's files (its program and its .info); a game whose folder is gone has none.
    ///
    /// Seen red 2026-10-19 with `read_game` leaving the size out (`size: None`): "its folder added
    /// up" (left: None, right: Some(391)).
    #[test]
    fn a_gog_games_size_is_its_folder_added_up() {
        let fx = fixtures::gog("gog_size");
        let games = read(&fx.installed);
        let info = std::fs::metadata(fx.forge.join("goggame-1207658924.info")).unwrap().len();
        assert_eq!(games[1].details.size, Some(1 + info), "its folder added up");
        assert_eq!(games[0].details.size, None, "a folder gone has no size");
    }

    /// NO WORKING DIRECTORY NAMED: the game starts in its own folder. A game with no .info is
    /// listed by the registry's name, without Play.
    ///
    /// Seen red 2026-10-18 with a game that has no .info counted as ready (`None => State::Ready`):
    /// left: ("Registry Name", Ready, true), right: ("Registry Name", ProgramGone, true).
    #[test]
    fn no_working_directory_and_no_info() {
        let folder = crate::test_temp::dir("gog_no_wd");
        let info = serde_json::json!({ "playTasks": [ { "isPrimary": true, "path": "game.exe", "type": "FileTask" } ] });
        assert_eq!(primary_task(&info, &folder), Some(PlayTask { exe: folder.join("game.exe"), args: String::new(), dir: folder.to_path_buf() }));
        let games = read(&[Installed { id: "77".into(), folder: folder.to_path_buf(), name: "Registry Name".into() }]);
        assert_eq!((games[0].name.as_str(), games[0].state, games[0].play.is_none()), ("Registry Name", State::ProgramGone, true));
    }
}
