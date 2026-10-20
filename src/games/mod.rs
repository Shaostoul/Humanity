//! The Games library's readers (2026-10-18; the operator, docs/PRIORITIES.md 2026-10-13: "A Games
//! library: list and launch every installed game (Steam, Epic, GOG, anything installed elsewhere),
//! the stores still doing downloads and updates behind it"). The page is the Apps page's red part
//! since 2026-10-19: src/gui/pages/apps.rs, its games in apps/games.rs.
//!
//! WHAT MAY BE DONE is set by the dated finding
//! docs/reference/findings/2026-10-13-game-stores-and-a-games-library.md, "What it means for
//! HumanityOS", and this module does exactly that and no more:
//! - READ each store's own install records, never writing to them: Steam's appmanifest files and
//!   libraryfolders.vdf, Epic's `.item` manifests, each GOG game's `goggame-<id>.info` (found
//!   through GOG's registry key, read with KEY_READ only).
//! - START a game only when the person presses Play, through the store's own documented route:
//!   `steam://rungameid/<appid>`, Epic's `?action=launch` link, a GOG game's own primary program as
//!   its `.info` declares it, any other game's own program. That is what a desktop shortcut does.
//! - Leave downloads, updates and licence checks with the stores: there is no Install or Update
//!   here, only a button that opens the store's own app. No store sign-in, no SteamCMD, no store
//!   protocols reimplemented, no owned-but-not-installed games, no polling and nothing on a timer:
//!   Steam's terms bar "non-human-controlled systems" from interacting with it "in any manner".
//!
//! Its second finding, docs/reference/findings/2026-10-19-games-page-art-stats-and-links.md
//! (pictures, store names, play records, links), adds: Steam's cached pictures read in place and
//! shown to this person only, never copied, sent or fetched; no store logos; the person's own Steam
//! play records read for ONE account, the one in use, and never written; GOG GALAXY's database
//! never read; links opened only on a click, never fetched in the background; no descriptions or
//! genres fetched from anywhere.
//!
//! - `keyvalues`: Valve's KeyValues text format; `binary_keyvalues`: the same as bytes, the
//!   shape of Steam's achievement records.
//! - `steam`, `epic`, `gog`: one reader each. Each is handed the folders to look in (`Sources`) so
//!   the tests point them at fixture files; `Sources::this_computer` finds the real ones.
//! - `other`: the games the person added by their own program, kept in the app's settings.
//! - A game's DETAILS (2026-10-19, [`Details`]), read with it on the same worker: its size and
//!   when it was updated (each store's own record; a GOG game's folder added up), and for a Steam
//!   game the person's own playtime and last-played time (`steam_user`), their achievements
//!   (`achievements`), their screenshots (`steam_user`) and their Workshop mods (`workshop`).
//!   Everything is from files already on this computer: nothing is fetched, and nothing a store
//!   keeps on its servers (an achievement's picture, a mod's title) is shown.
//! - `links`: the store-app and web links a game's details offer, each opened only on a click.
//! - `art`: a game's pictures: the ones Steam's own client cached on this computer, read in place,
//!   and an Epic or GOG game's program icon; never copied, sent or fetched (see its header).
//! - `epic_covers` (2026-10-19): the addresses of an Epic game's covers, from the launcher's own
//!   catalog on this computer, and the one function that fetches one, used only when the person
//!   turns on the Apps page's switch (off by default); nothing fetched is ever written anywhere.
//! - `launch`: the ONE way anything here is started or shown in a folder. Test builds replace it
//!   with a recorder, so no test can start a game, a store or a link.
//!
//! DESKTOP ONLY: a website cannot read the games on the computer it runs on.

pub mod achievements;
pub mod art;
pub mod binary_keyvalues;
pub mod epic;
pub mod epic_covers;
pub mod gog;
pub mod keyvalues;
pub mod launch;
pub mod links;
pub mod other;
// The Tools page reads Windows' installed-programs list through it too (software/records.rs).
#[cfg(windows)]
pub(crate) mod registry;
pub mod steam;
pub mod steam_user;
pub mod workshop;

use std::path::{Path, PathBuf};

pub use launch::{launch, Launch};
pub use other::AddedApp;

/// Where a game came from. Named in plain words only: no store's logo or branding is used (the
/// finding: trademarks were not researched, and a plain name describes where a game came from
/// without implying any partnership).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Store {
    Steam,
    Epic,
    Gog,
    /// Added by the person, by its own program.
    Added,
}

impl Store {
    /// The words for it on the page.
    pub fn name(self) -> &'static str {
        match self {
            Store::Steam => "Steam",
            Store::Epic => "Epic Games Store",
            Store::Gog => "GOG",
            Store::Added => "Added by you",
        }
    }
}

/// Whether a game can be started now, and if not, why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Installed: Play is offered.
    Ready,
    /// The store is still installing it (Steam's StateFlags without "fully installed", Epic's
    /// `bIsIncompleteInstall`). No Play: the store finishes it, in its own app.
    BeingInstalled,
    /// The store is taking it off the computer.
    BeingRemoved,
    /// Its folder is not on the disk (moved, deleted, or on a drive not plugged in).
    FolderGone,
    /// Its folder is there but the program that starts it is not (or, for GOG, the file saying
    /// which program that is).
    ProgramGone,
}

/// One game, as the page shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Game {
    pub store: Store,
    /// The store's own id (Steam's appid, Epic's AppName, GOG's game id), or for an added game its
    /// program's path.
    pub id: String,
    pub name: String,
    /// The folder it is installed in.
    pub folder: PathBuf,
    /// Whether that folder was there when the game was read (a game the store is still installing
    /// may not have one yet). Show in folder is offered only when it was.
    pub folder_here: bool,
    pub state: State,
    /// What Play does, when the game can be started now (only when `state` is `Ready`).
    pub play: Option<Launch>,
    /// What Show in folder shows: the game's folder, or an added game's own program in its folder.
    pub show: PathBuf,
    /// What else is known about it (read with it, on the worker).
    pub details: Details,
}

/// What else is known about a game (2026-10-19), from the stores' records on this computer and
/// read with the game on the page's worker thread. Each is `None` where its store keeps no such
/// record, or it did not read: the page then leaves it out rather than show a guess.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Details {
    /// Its size on the disk, bytes: Steam's `SizeOnDisk`, Epic's `InstallSize`, a GOG game's
    /// folder added up (`folder_size`).
    pub size: Option<u64>,
    /// When its store last updated it (Unix seconds): Steam's `LastUpdated`.
    pub updated: Option<u64>,
    /// How long the person has played it, minutes: Steam's `Playtime` (steam_user.rs).
    pub played_minutes: Option<u64>,
    /// When the person last played it (Unix seconds).
    pub last_played: Option<u64>,
    /// Its version as its store names it: Epic's `AppVersionString`.
    pub version: Option<String>,
    /// Its achievements, as the person has them (achievements.rs).
    pub achievements: Option<achievements::Achievements>,
    /// The person's screenshots of it (steam_user.rs).
    pub screenshots: Option<steam_user::Screenshots>,
    /// Its Workshop mods (workshop.rs).
    pub workshop: Option<workshop::Workshop>,
    /// Its pictures on this computer (art.rs).
    pub art: art::Art,
}

/// A store's own button for a game: its words, its hover words, and what it opens.
pub struct StoreButton {
    pub label: &'static str,
    pub tip: &'static str,
    pub open: Launch,
}

/// "Open in Steam" on a Steam game.
pub const OPEN_IN_STEAM: &str = "Open in Steam";
/// "Open in the Epic Games Store" on an Epic game.
pub const OPEN_IN_EPIC: &str = "Open in the Epic Games Store";

impl Game {
    /// The store's own button, where the store documents a link that opens it. Steam's opens the
    /// game's page in the person's Steam library; Epic's opens the store. GOG has none: GOG does
    /// not document a link or a command line for GOG GALAXY (the finding, 3.3), so none is sent.
    pub fn store_button(&self) -> Option<StoreButton> {
        match self.store {
            Store::Steam => Some(StoreButton {
                label: OPEN_IN_STEAM,
                tip: "Shows it in Steam, where its downloads, updates and settings are.",
                open: Launch::Link(format!("steam://nav/games/details/{}", self.id)),
            }),
            Store::Epic => Some(StoreButton {
                label: OPEN_IN_EPIC,
                tip: "Opens the Epic Games Store, where its downloads and updates are.",
                open: Launch::Link("com.epicgames.launcher://store".to_string()),
            }),
            Store::Gog | Store::Added => None,
        }
    }
}

/// What the readers found. A store that is not installed on this computer is `None` (the page
/// has no section for it); one that is installed with no games is `Some` of an empty list.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Found {
    pub steam: Option<Vec<Game>>,
    pub epic: Option<Vec<Game>>,
    pub gog: Option<Vec<Game>>,
}

/// Where the readers look: found on this computer by `this_computer`, or a test's fixture folders.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sources {
    /// Steam's own folder (the registry's `SteamPath`), whose `steamapps\libraryfolders.vdf`
    /// names every library folder.
    pub steam_root: Option<PathBuf>,
    /// Epic's manifests folder (`%ProgramData%\Epic\EpicGamesLauncher\Data\Manifests`).
    pub epic_manifests: Option<PathBuf>,
    /// GOG's installed games (each one's id, folder and name), from its registry key; `None` when
    /// that key is not there.
    pub gog_games: Option<Vec<gog::Installed>>,
    /// The Steam account signed in to Steam right now, when Steam is running (the registry's
    /// `ActiveProcess > ActiveUser`): whose records are read (steam_user.rs).
    pub steam_active_user: Option<u32>,
}

impl Sources {
    /// Find the stores' records on this computer, reading only: the registry with KEY_READ, and
    /// whether a folder exists. Windows only; elsewhere nothing is found.
    pub fn this_computer() -> Self {
        #[cfg(windows)]
        {
            Self {
                steam_root: steam::root_on_this_computer(),
                epic_manifests: epic::manifests_on_this_computer(),
                gog_games: gog::installed_on_this_computer(),
                steam_active_user: steam::active_user_on_this_computer(),
            }
        }
        #[cfg(not(windows))]
        {
            Self::default()
        }
    }
}

/// Read every store `sources` points at. Slow enough (a disk read per game) that the page runs it
/// on a worker thread, when the page opens and when Refresh is pressed, never every frame.
pub fn read(sources: &Sources) -> Found {
    Found {
        steam: sources.steam_root.as_deref().filter(|p| p.is_dir()).map(|root| steam::read(root, sources.steam_active_user)),
        epic: sources.epic_manifests.as_deref().filter(|p| p.is_dir()).map(epic::read),
        gog: sources.gog_games.as_deref().map(gog::read),
    }
}

/// A game's state from its folder and (where one is known) its program: `FolderGone`,
/// `ProgramGone`, or `ready` as given.
pub(crate) fn on_disk(folder: &Path, program: Option<&Path>, ready: State) -> State {
    if !folder.is_dir() {
        State::FolderGone
    } else if program.is_some_and(|p| !p.is_file()) {
        State::ProgramGone
    } else {
        ready
    }
}

/// Sort games by name the way a person reads a list: letters' case ignored.
pub(crate) fn sort_by_name(games: &mut [Game]) {
    games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.id.cmp(&b.id)));
}

/// Read a store's small record file, refusing anything larger than a record could be.
pub(crate) fn read_small(path: &Path) -> Option<String> {
    const MAX: u64 = 4 * 1024 * 1024;
    let bytes = read_bytes(path, MAX)?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Read a store's file as bytes, refusing one larger than `max`.
pub(crate) fn read_bytes(path: &Path, max: u64) -> Option<Vec<u8>> {
    note_disk_read();
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > max {
        return None;
    }
    std::fs::read(path).ok()
}

/// What is in a folder (nothing when it is not there or does not read).
pub(crate) fn list_dir(path: &Path) -> Vec<std::fs::DirEntry> {
    note_disk_read();
    std::fs::read_dir(path).map(|d| d.flatten().collect()).unwrap_or_default()
}

/// At most this many files and folders are counted for a folder's size.
pub const FOLDER_SIZE_LIMIT: usize = 200_000;

/// The size of everything in `folder`, added up on the worker thread (a GOG game's: GOG records
/// none). `None` when it holds more than [`FOLDER_SIZE_LIMIT`] files and folders (a number cut
/// off there would be wrong) or is not there. Links (symbolic, junctions) are not followed.
pub fn folder_size(folder: &Path) -> Option<u64> {
    folder_size_within(folder, FOLDER_SIZE_LIMIT)
}

fn folder_size_within(folder: &Path, limit: usize) -> Option<u64> {
    if !folder.is_dir() {
        return None;
    }
    let (mut total, mut seen) = (0u64, 0usize);
    let mut todo = vec![folder.to_path_buf()];
    while let Some(dir) = todo.pop() {
        for entry in list_dir(&dir) {
            seen += 1;
            if seen > limit {
                return None;
            }
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_dir() {
                todo.push(entry.path());
            } else if kind.is_file() {
                total += entry.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    Some(total)
}

#[cfg(test)]
thread_local! {
    static DISK_READS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// A store's file or folder was read on this thread (test builds count them: the page's tests
/// check that drawing it reads nothing).
pub(crate) fn note_disk_read() {
    #[cfg(test)]
    DISK_READS.with(|c| c.set(c.get() + 1));
}

/// Test builds: how many store files and folders this thread has read.
#[cfg(test)]
pub(crate) fn disk_reads_here() -> u64 {
    DISK_READS.with(|c| c.get())
}

#[cfg(test)]
mod folder_size_tests {
    /// A FOLDER'S SIZE: its files added up, in folders inside folders; nothing past the limit (a
    /// number cut off there would be wrong); nothing for a folder that is not there.
    ///
    /// Seen red 2026-10-19 with the limit check returning the total so far (`return Some(total)`):
    /// "past the limit it says nothing" got Some(3).
    #[test]
    fn a_folders_size() {
        let dir = crate::test_temp::dir("folder_size");
        std::fs::create_dir_all(dir.join("a").join("b")).unwrap();
        std::fs::write(dir.join("one.bin"), [0u8; 3]).unwrap();
        std::fs::write(dir.join("a").join("two.bin"), [0u8; 40]).unwrap();
        std::fs::write(dir.join("a").join("b").join("three.bin"), [0u8; 500]).unwrap();
        assert_eq!(super::folder_size(&dir), Some(543));
        assert_eq!(super::folder_size_within(&dir, 3), None, "past the limit it says nothing");
        assert_eq!(super::folder_size(&dir.join("gone")), None);
    }
}

#[cfg(test)]
pub(crate) mod fixtures;

/// The readers against THIS computer's real stores, printed for a look by hand (2026-10-18; the
/// other tests read only fixture folders):
/// `node scripts/lib-tests.js games::this_computer --ignored --nocapture`. Ignored because what it
/// finds depends on the machine. It reads only, as the page does, and starts nothing. Since
/// 2026-10-19 it prints each game's details too: size, updated, playtime, last played,
/// achievements, screenshots and Workshop items, as the page would show them.
#[cfg(test)]
mod this_computer {
    use super::Game;

    fn details(g: &Game) -> String {
        let d = &g.details;
        let n = |v: Option<u64>| v.map(|v| v.to_string()).unwrap_or_else(|| "-".into());
        let ach = match &d.achievements {
            None => "-".to_string(),
            Some(a) => match a.unlocked() {
                Some(u) => format!("{u} of {}", a.total()),
                None => format!("{} (not known which)", a.total()),
            },
        };
        let shots = d.screenshots.as_ref().map(|s| s.total.to_string()).unwrap_or_else(|| "-".into());
        let mods = d.workshop.as_ref().map(|w| format!("{} ({} bytes)", w.items.len(), w.total_size)).unwrap_or_else(|| "-".into());
        let art = &d.art;
        let has = |p: &Option<std::path::PathBuf>| if p.is_some() { "yes" } else { "-" };
        let icon = art.icon.as_ref().map(|i| format!("{}x{}", i.width, i.height)).unwrap_or_else(|| "-".into());
        let epic = art.epic_covers.as_ref().map(|c| format!("{:?}", c.urls())).unwrap_or_else(|| "-".into());
        format!(
            "size {} | updated {} | played {} min | last {} | version {} | achievements {ach} | screenshots {shots} | workshop {mods} | cover {} hero {} logo {} icon {icon} | Epic covers (addresses only) {epic}",
            n(d.size),
            n(d.updated),
            n(d.played_minutes),
            n(d.last_played),
            d.version.as_deref().unwrap_or("-"),
            has(&art.cover),
            has(&art.hero),
            has(&art.logo)
        )
    }

    #[test]
    #[ignore]
    fn print_what_the_page_would_list_here() {
        let sources = super::Sources::this_computer();
        if let Some(root) = &sources.steam_root {
            println!("Steam account read: {:?} (signed in now: {:?})", super::steam_user::account(root, sources.steam_active_user).map(|a| a.id), sources.steam_active_user);
        }
        let found = super::read(&sources);
        for (store, games) in [("Steam", &found.steam), ("Epic Games Store", &found.epic), ("GOG", &found.gog)] {
            match games {
                None => println!("{store}: not installed here"),
                Some(list) => {
                    println!("{store}: {} game(s)", list.len());
                    for g in list {
                        println!("  {} | {:?} | play: {} | {}", g.name, g.state, g.play.is_some(), g.folder.display());
                        println!("      {}", details(g));
                    }
                }
            }
        }
    }
}
