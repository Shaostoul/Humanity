//! Steam's installed games, read from Steam's own files and never written to.
//!
//! Steam keeps its folder in the registry (`HKCU\Software\Valve\Steam`, value `SteamPath`). In it,
//! `steamapps\libraryfolders.vdf` names every library folder the person made (on other drives,
//! say), and each library's `steamapps` folder holds one `appmanifest_<appid>.acf` per game: its
//! appid, name, the folder under `steamapps\common` it is installed in, and `StateFlags`, which
//! say whether Steam has finished installing it. Valve's Steamworks documentation describes the
//! appmanifest as showing "the current install state of this app (KeyValues)" (the finding, 1.4).
//!
//! Play sends `steam://rungameid/<appid>`, which Valve's own documentation and its wiki describe
//! (the finding, 1.2 and 1.3): Steam starts the game, updating it first if it has to. Nothing here
//! talks to Steam's servers or drives Steam's client beyond that one link per click.
//!
//! A game's details (2026-10-19) are read with it: from its appmanifest `SizeOnDisk`,
//! `LastUpdated` and `LastPlayed`; from the person's own records the rest (steam_user.rs for
//! playtime and screenshots, achievements.rs, workshop.rs).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::keyvalues::{self, Value};
use super::{on_disk, read_small, sort_by_name, Details, Game, Launch, State, Store};

/// Steamworks Common Redistributables: installed beside games by Steam itself, not a game.
pub const REDISTRIBUTABLES: &str = "228980";

/// `StateFlags` bits (Steam's EAppState): the game is fully installed; Steam is uninstalling it.
const FULLY_INSTALLED: u64 = 4;
const UNINSTALLING: u64 = 2048;

/// What Steam's `StateFlags` say about a game. Fully installed is playable (Steam may still have
/// an update for it, which it applies when the game is started); without that bit Steam is still
/// installing it, so there is no Play until it is done.
pub fn state_of(flags: u64) -> State {
    if flags & UNINSTALLING != 0 {
        State::BeingRemoved
    } else if flags & FULLY_INSTALLED != 0 {
        State::Ready
    } else {
        State::BeingInstalled
    }
}

/// Play's link for a Steam game.
pub fn play_link(appid: &str) -> String {
    format!("steam://rungameid/{appid}")
}

/// Every library folder: Steam's own folder first (it is always one, and the old file format did
/// not list it), then each one `libraryfolders.vdf` names, each once.
pub fn libraries(steam_root: &Path) -> Vec<PathBuf> {
    let mut out = vec![steam_root.to_path_buf()];
    let vdf = steam_root.join("steamapps").join("libraryfolders.vdf");
    let Some(text) = read_small(&vdf) else { return out };
    let Ok(kv) = keyvalues::parse(&text) else { return out };
    let Some(folders) = keyvalues::find(&kv, "libraryfolders").and_then(Value::block) else { return out };
    for (key, value) in folders {
        // Libraries are numbered "0", "1", ...; the other entries (the old format's
        // "TimeNextStatsReport", say) are not libraries.
        if key.is_empty() || !key.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        // The current format: a block with its "path". The old one: the path itself.
        let path = match value {
            Value::Text(p) => Some(p.as_str()),
            Value::Block(b) => keyvalues::find_text(b, "path"),
        };
        let Some(path) = path.map(str::trim).filter(|p| !p.is_empty()) else { continue };
        let path = PathBuf::from(path);
        if !out.iter().any(|p| same_folder(p, &path)) {
            out.push(path);
        }
    }
    out
}

/// Two spellings of one folder: separators and letter case aside (Windows paths).
fn same_folder(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| p.to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_lowercase();
    norm(a) == norm(b)
}

/// The appid in an `appmanifest_<appid>.acf` file name, when it is one.
fn manifest_appid(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?.to_ascii_lowercase();
    let id = name.strip_prefix("appmanifest_")?.strip_suffix(".acf")?;
    (!id.is_empty() && id.chars().all(|c| c.is_ascii_digit())).then(|| id.to_string())
}

/// Every Steam game installed in the libraries Steam's folder names, sorted by name, each with its
/// details (2026-10-19): from its appmanifest its size and when Steam last updated it, and from
/// the person's own records (steam_user.rs, achievements.rs, workshop.rs) their playtime, when
/// they last played it, their achievements, their screenshots and their Workshop mods: those of
/// ONE account only, `active_user` (signed in now) or the one Steam says signed in last
/// (steam_user.rs `account`), never several people's mixed on a shared computer.
pub fn read(steam_root: &Path, active_user: Option<u32>) -> Vec<Game> {
    let mut out = Vec::new();
    for (apps, mut game) in manifests(steam_root) {
        game.details.workshop = super::workshop::read(&apps, &game.id);
        out.push(game);
    }
    let account = super::steam_user::account(steam_root, active_user);
    let played = account.as_ref().map(super::steam_user::playtimes).unwrap_or_default();
    let stats = steam_root.join("appcache").join("stats");
    for game in &mut out {
        if let Some(p) = played.get(&game.id) {
            game.details.played_minutes = p.minutes;
            // The person's own record of when they last played it, over the manifest's.
            game.details.last_played = p.last.or(game.details.last_played);
        }
        if let Some(account) = &account {
            game.details.screenshots = super::steam_user::screenshots(account, &game.id);
        }
        game.details.achievements = super::achievements::read(&stats, account.as_ref().map(|a| a.id), &game.id);
        game.details.art = super::art::steam(steam_root, &game.id);
    }
    sort_by_name(&mut out);
    out
}

/// Every app Steam has installed in the libraries Steam's folder names, each once, with its
/// `steamapps` folder: its appmanifest only, none of the person's records.
fn manifests(steam_root: &Path) -> Vec<(PathBuf, Game)> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for library in libraries(steam_root) {
        let apps = library.join("steamapps");
        let mut manifests: Vec<(String, PathBuf)> = super::list_dir(&apps).into_iter().filter_map(|e| manifest_appid(&e.path()).map(|id| (id, e.path()))).collect();
        manifests.sort();
        for (file_id, path) in manifests {
            if let Some(game) = read_manifest(&path, &file_id, &apps) {
                if seen.insert(game.id.clone()) {
                    out.push((apps.clone(), game));
                }
            }
        }
    }
    out
}

/// Every app Steam has installed, from its appmanifests alone (2026-10-19, for the Tools page: a
/// program Steam installed, Blender say, counts as installed). None of the person's own records
/// (playtime, achievements, screenshots, mods) and none of the pictures are read.
pub fn installed(steam_root: &Path) -> Vec<Game> {
    let mut out: Vec<Game> = manifests(steam_root).into_iter().map(|(_, g)| g).collect();
    sort_by_name(&mut out);
    out
}

/// One appmanifest. `None` for a file that does not read, names another appid than its own file
/// name, or is Steam's redistributables.
fn read_manifest(path: &Path, file_id: &str, steamapps: &Path) -> Option<Game> {
    let kv = keyvalues::parse(&read_small(path)?).ok()?;
    let app = keyvalues::find(&kv, "AppState")?.block()?;
    let appid = keyvalues::find_text(app, "appid").map(str::trim).unwrap_or(file_id);
    if appid != file_id || appid == REDISTRIBUTABLES {
        return None;
    }
    let installdir = keyvalues::find_text(app, "installdir").map(str::trim).unwrap_or("");
    let name = keyvalues::find_text(app, "name").map(str::trim).filter(|n| !n.is_empty()).map(str::to_string).unwrap_or_else(|| {
        if installdir.is_empty() {
            format!("Steam game {appid}")
        } else {
            installdir.to_string()
        }
    });
    let flags = keyvalues::find_text(app, "StateFlags").and_then(|f| f.trim().parse::<u64>().ok()).unwrap_or(0);
    let folder = steamapps.join("common").join(installdir);
    let state = match state_of(flags) {
        // A folder that should be there and is not; an empty installdir names no folder at all.
        State::Ready if installdir.is_empty() => State::FolderGone,
        State::Ready => on_disk(&folder, None, State::Ready),
        other => other,
    };
    let play = (state == State::Ready).then(|| Launch::Link(play_link(appid)));
    let number = |key: &str| keyvalues::find_text(app, key).and_then(|t| t.trim().parse::<u64>().ok()).filter(|n| *n > 0);
    let details = Details { size: number("SizeOnDisk"), updated: number("LastUpdated"), last_played: number("LastPlayed"), ..Details::default() };
    Some(Game { store: Store::Steam, id: appid.to_string(), name, show: folder.clone(), folder_here: folder.is_dir(), folder, state, play, details })
}

/// Steam's folder on this computer, from the registry (read only), when it is there. Steam writes
/// it with forward slashes ("c:/program files (x86)/steam"); they are turned to backslashes so the
/// folders shown read as Windows paths.
#[cfg(windows)]
pub fn root_on_this_computer() -> Option<PathBuf> {
    let key = super::registry::open(super::registry::Root::CurrentUser, r"Software\Valve\Steam")?;
    let path = key.string("SteamPath")?;
    let path = PathBuf::from(path.trim().replace('/', "\\"));
    path.is_dir().then_some(path)
}

/// The Steam account signed in right now, from the registry (read only): Steam writes its account
/// id to `HKCU\Software\Valve\Steam\ActiveProcess`, value `ActiveUser`, while it runs, and 0
/// when nobody is signed in. `None` when Steam is not running.
#[cfg(windows)]
pub fn active_user_on_this_computer() -> Option<u32> {
    let key = super::registry::open(super::registry::Root::CurrentUser, r"Software\Valve\Steam\ActiveProcess")?;
    key.dword("ActiveUser").filter(|id| *id != 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::fixtures;

    /// TWO LIBRARIES, A GAME STILL DOWNLOADING, A FOLDER GONE: Steam's folder and a second library
    /// named in libraryfolders.vdf are both read; a game whose StateFlags lack "fully installed" is
    /// "being installed" with no Play; one whose folder is not there says so with no Play; Steam's
    /// redistributables are not a game; and Play's link is exactly steam://rungameid/<appid>.
    ///
    /// Seen red 2026-10-18 with `libraries` returning only Steam's own folder: "the second library's
    /// game is read: [\"Deep Rover\", \"Orbital Farmstead\", \"Sunken Archive\"]" (Old Lighthouse, in
    /// the second library, missing).
    #[test]
    fn two_libraries_a_download_and_a_missing_folder() {
        let fx = fixtures::steam("steam_two_libs");
        let games = read(&fx.root, None);
        let names: Vec<&str> = games.iter().map(|g| g.name.as_str()).collect();
        assert!(names.contains(&"Old Lighthouse"), "the second library's game is read: {names:?}");
        assert_eq!(names, vec!["Deep Rover", "Old Lighthouse", "Orbital Farmstead", "Sunken Archive"], "sorted by name, the redistributables left out");

        let farm = games.iter().find(|g| g.name == "Orbital Farmstead").unwrap();
        assert_eq!(farm.state, State::Ready);
        assert_eq!(farm.play, Some(Launch::Link("steam://rungameid/1000010".into())), "Play is Steam's own link");
        assert_eq!(farm.folder, fx.root.join("steamapps").join("common").join("Orbital Farmstead"));

        let rover = games.iter().find(|g| g.name == "Deep Rover").unwrap();
        assert_eq!(rover.state, State::BeingInstalled, "StateFlags 1026 is still downloading");
        assert_eq!(rover.play, None, "no Play while Steam installs it");

        let sunk = games.iter().find(|g| g.name == "Sunken Archive").unwrap();
        assert_eq!(sunk.state, State::FolderGone, "its folder is not there");
        assert_eq!(sunk.play, None);

        let old = games.iter().find(|g| g.name == "Old Lighthouse").unwrap();
        assert_eq!(old.folder, fx.second_library.join("steamapps").join("common").join("Lighthouse"));
        assert_eq!(old.store_button().map(|b| b.open), Some(Launch::Link("steam://nav/games/details/1000030".into())), "Open in Steam shows its page in Steam");
    }

    /// A GAME'S DETAILS READ WITH IT: Orbital Farmstead's size and last update from its manifest,
    /// its playtime and last-played time from the person's record (which wins over the manifest's
    /// older time), its achievements, screenshots and Workshop; Old Lighthouse, in the second
    /// library, its size and playtime and nothing else; Deep Rover, never played, none of them.
    ///
    /// Seen red 2026-10-19 with the person's last-played time not read over the manifest's
    /// (`p.last.or(..)` swapped to `game.details.last_played.or(p.last)`): "the person's record
    /// wins" (left: Some(1789500000), right: Some(1790000000)).
    #[test]
    fn a_games_details_are_read_with_it() {
        let fx = fixtures::steam("steam_details");
        let games = read(&fx.root, None);
        let farm = &games.iter().find(|g| g.name == "Orbital Farmstead").unwrap().details;
        assert_eq!((farm.size, farm.updated), (Some(33_471_534_235), Some(1_791_000_000)));
        assert_eq!(farm.last_played, Some(1_790_000_000), "the person's record wins");
        assert_eq!(farm.played_minutes, Some(754));
        assert_eq!(farm.achievements.as_ref().map(|a| (a.total(), a.unlocked())), Some((5, Some(3))));
        assert_eq!(farm.screenshots.as_ref().map(|s| s.total), Some(3));
        assert_eq!(farm.workshop.as_ref().map(|w| w.items.len()), Some(3));
        let old = &games.iter().find(|g| g.name == "Old Lighthouse").unwrap().details;
        assert_eq!((old.size, old.played_minutes, old.last_played), (Some(2_147_483_648), Some(42), None));
        assert!(old.achievements.is_none() && old.screenshots.is_none() && old.workshop.is_none());
        let rover = &games.iter().find(|g| g.name == "Deep Rover").unwrap().details;
        assert_eq!((rover.played_minutes, rover.last_played), (None, None), "never played");
    }

    /// WHAT STEAM INSTALLED, AND NOTHING OF THE PERSON'S (2026-10-19, for the Tools page): the same
    /// games as `read`, by name, from the same two libraries, with none of the person's records
    /// (playtime, achievements, mods) and no pictures read.
    ///
    /// Seen red 2026-10-19 with `installed` returning `read(steam_root, None)`: "none of the
    /// person's records" failed.
    #[test]
    fn what_steam_installed_and_nothing_of_the_persons() {
        let fx = fixtures::steam("steam_installed");
        let all = installed(&fx.root);
        let names: Vec<&str> = all.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, vec!["Deep Rover", "Old Lighthouse", "Orbital Farmstead", "Sunken Archive"]);
        assert!(all.iter().all(|g| g.details.played_minutes.is_none() && g.details.achievements.is_none() && g.details.workshop.is_none() && g.details.art == Default::default()), "none of the person's records");
        assert_eq!(all.iter().find(|g| g.name == "Orbital Farmstead").map(|g| g.play.clone()), Some(Some(Launch::Link("steam://rungameid/1000010".into()))));
    }

    /// THE STATE FLAGS: fully installed is Ready even with an update waiting (Steam applies it when
    /// the game starts); without that bit it is being installed; uninstalling is being removed.
    ///
    /// Seen red 2026-10-18 with the fully-installed test reading `flags == 4`: "an update waiting is
    /// still playable" (left: BeingInstalled, right: Ready).
    #[test]
    fn state_flags() {
        assert_eq!(state_of(4), State::Ready);
        assert_eq!(state_of(6), State::Ready, "an update waiting is still playable");
        assert_eq!(state_of(1026), State::BeingInstalled);
        assert_eq!(state_of(0), State::BeingInstalled);
        assert_eq!(state_of(2052), State::BeingRemoved);
    }

    /// THE OLD FILE FORMAT, where each library is just its path, and a manifest whose appid is not
    /// its file's (or is not a number) is not read: an appid goes into a link, so only digits do.
    ///
    /// Seen red 2026-10-18 with the check that the appid is its file's dropped (`if appid ==
    /// REDISTRIBUTABLES` alone): "neither is read" (appmanifest_5.acf's "Wrong" was listed).
    #[test]
    fn the_old_format_and_odd_manifests() {
        let root = crate::test_temp::dir("steam_old_format");
        let other = crate::test_temp::dir("steam_old_format_lib");
        std::fs::create_dir_all(root.join("steamapps")).unwrap();
        let escaped = other.to_string_lossy().replace('\\', "\\\\");
        std::fs::write(root.join("steamapps").join("libraryfolders.vdf"), format!("\"LibraryFolders\"\n{{\n\t\"TimeNextStatsReport\"\t\t\"1700000000\"\n\t\"1\"\t\t\"{escaped}\"\n}}\n")).unwrap();
        assert_eq!(libraries(&root).len(), 2, "the old format's numbered path is a library");
        std::fs::write(root.join("steamapps").join("appmanifest_5.acf"), "\"AppState\" { \"appid\" \"6\" \"name\" \"Wrong\" \"StateFlags\" \"4\" \"installdir\" \"W\" }").unwrap();
        std::fs::write(root.join("steamapps").join("appmanifest_x1.acf"), "\"AppState\" { \"appid\" \"x1\" \"name\" \"Letters\" \"StateFlags\" \"4\" \"installdir\" \"L\" }").unwrap();
        assert!(read(&root, None).is_empty(), "neither is read");
    }
}
