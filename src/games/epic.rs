//! The Epic Games Store's installed games, read from its own manifests and never written to.
//!
//! The Epic Games Launcher keeps one `.item` file (JSON) per installed game in
//! `%ProgramData%\Epic\EpicGamesLauncher\Data\Manifests`: its `DisplayName`, `InstallLocation`,
//! `LaunchExecutable` (relative to that folder), and the three ids its launch link is built from,
//! `CatalogNamespace`, `CatalogItemId` and `AppName` (the same files Playnite reads: the finding,
//! section 4). A piece of DLC has its own file, naming its game in `MainGameAppName`; Unreal Engine
//! installs have files too. Neither is a game to start, so neither is listed.
//!
//! Play sends the link Epic documents for its launcher (the finding, 2.3):
//! `com.epicgames.launcher://apps/<CatalogNamespace>%3A<CatalogItemId>%3A<AppName>?action=launch&silent=true`.
//! The store then starts the game, updating it first if it has to.

use std::path::{Path, PathBuf};

use super::{on_disk, read_small, sort_by_name, Details, Game, Launch, State, Store};

/// Play's link for an Epic game: the three ids, each percent-encoded, joined by an encoded colon.
pub fn play_link(namespace: &str, catalog_item: &str, app_name: &str) -> String {
    format!("com.epicgames.launcher://apps/{}%3A{}%3A{}?action=launch&silent=true", encode(namespace), encode(catalog_item), encode(app_name))
}

/// Percent-encode everything but the characters a URL never needs encoded (RFC 3986 unreserved).
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Every game the Epic Games Store has installed (or is installing), sorted by name.
pub fn read(manifests: &Path) -> Vec<Game> {
    let mut files: Vec<PathBuf> = super::list_dir(manifests).into_iter().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("item"))).collect();
    files.sort();
    let mut out: Vec<Game> = Vec::new();
    for file in files {
        if let Some(game) = read_item(&file) {
            if !out.iter().any(|g| g.id == game.id) {
                out.push(game);
            }
        }
    }
    sort_by_name(&mut out);
    out
}

/// One manifest. `None` for a file that does not read, DLC, or an Unreal Engine install.
fn read_item(path: &Path) -> Option<Game> {
    let json: serde_json::Value = serde_json::from_str(&read_small(path)?).ok()?;
    let text = |key: &str| json.get(key).and_then(|v| v.as_str()).map(str::trim).unwrap_or("").to_string();
    let app_name = text("AppName");
    if app_name.is_empty() || app_name.starts_with("UE_") {
        return None;
    }
    let main = text("MainGameAppName");
    if !main.is_empty() && main != app_name {
        return None; // DLC: its game has its own manifest
    }
    let categories: Vec<String> = json.get("AppCategories").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|c| c.as_str().map(str::to_string)).collect()).unwrap_or_default();
    if categories.iter().any(|c| c == "engines" || c == "plugins" || c.starts_with("plugins/")) {
        return None;
    }
    let location = text("InstallLocation");
    if location.is_empty() {
        return None;
    }
    let folder = PathBuf::from(&location);
    let exe = text("LaunchExecutable");
    let program = (!exe.is_empty()).then(|| folder.join(&exe));
    let incomplete = json.get("bIsIncompleteInstall").and_then(|v| v.as_bool()).unwrap_or(false);
    let state = if incomplete { State::BeingInstalled } else { on_disk(&folder, program.as_deref(), State::Ready) };
    let (namespace, catalog_item) = (text("CatalogNamespace"), text("CatalogItemId"));
    // Without its three ids there is no link to send: it is listed, and started from the store.
    let play = (state == State::Ready && !namespace.is_empty() && !catalog_item.is_empty()).then(|| Launch::Link(play_link(&namespace, &catalog_item, &app_name)));
    let name = Some(text("DisplayName")).filter(|n| !n.is_empty()).unwrap_or_else(|| app_name.clone());
    // Its details (2026-10-19): the size Epic recorded for it and the version it names.
    let size = json.get("InstallSize").and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))).filter(|n| *n > 0);
    let version = Some(text("AppVersionString")).filter(|v| !v.is_empty());
    let details = Details { size, version, art: super::art::program(Store::Epic, program.as_deref()), ..Details::default() };
    Some(Game { store: Store::Epic, id: app_name, name, show: folder.clone(), folder_here: folder.is_dir(), folder, state, play, details })
}

/// The manifests folder on this computer, when the Epic Games Launcher has made it.
#[cfg(windows)]
pub fn manifests_on_this_computer() -> Option<PathBuf> {
    let data = std::env::var_os("ProgramData").or_else(|| std::env::var_os("ALLUSERSPROFILE")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
    let dir = data.join("Epic").join("EpicGamesLauncher").join("Data").join("Manifests");
    dir.is_dir().then_some(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::fixtures;

    /// EPIC'S MANIFESTS: an installed game with Play's link exactly as Epic documents it, one still
    /// installing (no Play), and DLC, an Unreal Engine install and a damaged file left out.
    ///
    /// Seen red 2026-10-18 with the DLC check taken out: "only the two games are listed" found
    /// ["Lantern Valley", "Lantern Valley: Night Market", "Night Courier"].
    #[test]
    fn epic_manifests() {
        let fx = fixtures::epic("epic_manifests");
        let games = read(&fx.manifests);
        let names: Vec<&str> = games.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, vec!["Lantern Valley", "Night Courier"], "only the two games are listed");
        let lantern = &games[0];
        assert_eq!(lantern.state, State::Ready);
        assert_eq!(lantern.folder, fx.lantern);
        assert_eq!(lantern.play, Some(Launch::Link("com.epicgames.launcher://apps/ns1%3Aitem1%3ALantern?action=launch&silent=true".into())), "Play is Epic's own launch link");
        assert_eq!(lantern.store_button().map(|b| b.open), Some(Launch::Link("com.epicgames.launcher://store".into())), "Open in the Epic Games Store opens the store");
        let courier = &games[1];
        assert_eq!(courier.state, State::BeingInstalled, "bIsIncompleteInstall is still installing");
        assert_eq!(courier.play, None);
    }

    /// AN EPIC GAME'S DETAILS (2026-10-19): the size Epic recorded (`InstallSize`) and the version
    /// it names (`AppVersionString`); a manifest without them has neither.
    ///
    /// Seen red 2026-10-19 with InstallSize read only as text (`as_str`): "its size" (left: None,
    /// right: Some(8589934592)), Epic writing it as a number.
    #[test]
    fn an_epic_games_details() {
        let fx = fixtures::epic("epic_details");
        let games = read(&fx.manifests);
        assert_eq!(games[0].details.size, Some(8_589_934_592), "its size");
        assert_eq!(games[0].details.version.as_deref(), Some("2.4.1-CL-118"));
        let bare = crate::test_temp::dir("epic_details_bare");
        std::fs::create_dir_all(bare.join("game")).unwrap();
        let item = serde_json::json!({ "AppName": "Bare", "DisplayName": "Bare", "InstallLocation": bare.join("game").to_string_lossy(), "AppCategories": ["games"] });
        std::fs::write(bare.join("x.item"), item.to_string()).unwrap();
        let bare_games = read(&bare);
        assert_eq!((bare_games[0].details.size, bare_games[0].details.version.as_deref()), (None, None));
    }

    /// A PROGRAM GONE FROM ITS FOLDER, and an id with odd characters encoded, never passed raw into
    /// the link.
    ///
    /// Seen red 2026-10-18 with `encode` passing every character through: left:
    /// "com.epicgames.launcher://apps/a b%3Ac&d%3Ae?f?action=launch&silent=true", right:
    /// "com.epicgames.launcher://apps/a%20b%3Ac%26d%3Ae%3Ff?action=launch&silent=true".
    #[test]
    fn a_program_gone_and_an_odd_id() {
        let fx = fixtures::epic("epic_gone");
        std::fs::remove_file(fx.lantern.join("Binaries").join("Win64").join("LanternValley.exe")).unwrap();
        let games = read(&fx.manifests);
        assert_eq!(games[0].state, State::ProgramGone, "its program is not in its folder");
        assert_eq!(games[0].play, None);
        assert_eq!(play_link("a b", "c&d", "e?f"), "com.epicgames.launcher://apps/a%20b%3Ac%26d%3Ae%3Ff?action=launch&silent=true");
    }
}
