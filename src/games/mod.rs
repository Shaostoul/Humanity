//! The Games library's readers (2026-10-18; the operator, docs/PRIORITIES.md 2026-10-13: "A Games
//! library: list and launch every installed game (Steam, Epic, GOG, anything installed elsewhere),
//! the stores still doing downloads and updates behind it"). The page is
//! src/gui/pages/games.rs.
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
//! - `keyvalues`: Valve's KeyValues text format.
//! - `steam`, `epic`, `gog`: one reader each. Each is handed the folders to look in (`Sources`) so
//!   the tests point them at fixture files; `Sources::this_computer` finds the real ones.
//! - `other`: the games the person added by their own program, kept in the app's settings.
//! - `launch`: the ONE way anything here is started or shown in a folder. Test builds replace it
//!   with a recorder, so no test can start a game, a store or a link.
//!
//! DESKTOP ONLY: a website cannot read the games on the computer it runs on.

pub mod epic;
pub mod gog;
pub mod keyvalues;
pub mod launch;
pub mod other;
#[cfg(windows)]
mod registry;
pub mod steam;

use std::path::{Path, PathBuf};

pub use launch::{launch, Launch};
pub use other::AddedGame;

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
        steam: sources.steam_root.as_deref().filter(|p| p.is_dir()).map(steam::read),
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
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
pub(crate) mod fixtures;

/// The readers against THIS computer's real stores, printed for a look by hand (2026-10-18; the
/// other tests read only fixture folders):
/// `node scripts/lib-tests.js games::this_computer --ignored --nocapture`. Ignored because what it
/// finds depends on the machine. It reads only, as the page does, and starts nothing.
#[cfg(test)]
mod this_computer {
    #[test]
    #[ignore]
    fn print_what_the_page_would_list_here() {
        let found = super::read(&super::Sources::this_computer());
        for (store, games) in [("Steam", &found.steam), ("Epic Games Store", &found.epic), ("GOG", &found.gog)] {
            match games {
                None => println!("{store}: not installed here"),
                Some(list) => {
                    println!("{store}: {} game(s)", list.len());
                    for g in list {
                        println!("  {} | {:?} | play: {} | {}", g.name, g.state, g.play.is_some(), g.folder.display());
                    }
                }
            }
        }
    }
}
