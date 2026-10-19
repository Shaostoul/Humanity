// Test builds only (games/mod.rs): the stores' install records, written into a scratch folder the
// way each store writes them, for the readers' tests, the Games page's tests and its snapshot.
// Nothing here reads this computer's own stores.

use std::path::PathBuf;

use super::gog::Installed;
use super::Sources;

/// A Steam folder with two libraries.
pub(crate) struct SteamFixture {
    pub _scratch: crate::test_temp::TempPath,
    /// Steam's own folder (its first library).
    pub root: PathBuf,
    /// The second library, on "another drive", named in libraryfolders.vdf.
    pub second_library: PathBuf,
}

fn write(path: PathBuf, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn manifest(appid: &str, name: &str, flags: &str, installdir: &str) -> String {
    format!("\"AppState\"\n{{\n\t\"appid\"\t\t\"{appid}\"\n\t\"universe\"\t\t\"1\"\n\t\"name\"\t\t\"{name}\"\n\t\"StateFlags\"\t\t\"{flags}\"\n\t\"installdir\"\t\t\"{installdir}\"\n\t\"SizeOnDisk\"\t\t\"1000\"\n}}\n")
}

/// Steam: Orbital Farmstead installed; Deep Rover still downloading (StateFlags 1026); Sunken
/// Archive installed but its folder gone; Steam's redistributables; and in the second library Old
/// Lighthouse, installed in "Lighthouse".
pub(crate) fn steam(tag: &str) -> SteamFixture {
    let scratch = crate::test_temp::dir(tag);
    let root = scratch.join("Steam");
    let second = scratch.join("SteamLibrary");
    let esc = |p: &PathBuf| p.to_string_lossy().replace('\\', "\\\\");
    let vdf = format!(
        "\"libraryfolders\"\n{{\n\t\"0\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t\t\"label\"\t\t\"\"\n\t\t\"apps\"\n\t\t{{\n\t\t\t\"1000010\"\t\t\"1000\"\n\t\t}}\n\t}}\n\t\"1\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t\t\"label\"\t\t\"\"\n\t}}\n}}\n",
        esc(&root),
        esc(&second)
    );
    write(root.join("steamapps").join("libraryfolders.vdf"), &vdf);
    let apps = root.join("steamapps");
    write(apps.join("appmanifest_1000010.acf"), &manifest("1000010", "Orbital Farmstead", "4", "Orbital Farmstead"));
    write(apps.join("common").join("Orbital Farmstead").join("farmstead.exe"), "x");
    write(apps.join("appmanifest_1000020.acf"), &manifest("1000020", "Deep Rover", "1026", "Deep Rover"));
    write(apps.join("appmanifest_1000040.acf"), &manifest("1000040", "Sunken Archive", "4", "Sunken Archive"));
    write(apps.join("appmanifest_228980.acf"), &manifest("228980", "Steamworks Common Redistributables", "4", "Steamworks Shared"));
    write(apps.join("common").join("Steamworks Shared").join("x.txt"), "x");
    let apps2 = second.join("steamapps");
    write(apps2.join("appmanifest_1000030.acf"), &manifest("1000030", "Old Lighthouse", "4", "Lighthouse"));
    write(apps2.join("common").join("Lighthouse").join("lighthouse.exe"), "x");
    SteamFixture { _scratch: scratch, root, second_library: second }
}

/// An Epic manifests folder and the games' folders.
pub(crate) struct EpicFixture {
    pub _scratch: crate::test_temp::TempPath,
    pub manifests: PathBuf,
    /// Lantern Valley's folder.
    pub lantern: PathBuf,
}

fn item(display: &str, location: &PathBuf, exe: &str, ns: &str, catalog: &str, app: &str, main: &str, incomplete: bool, categories: &str) -> String {
    serde_json::json!({
        "FormatVersion": 0,
        "bIsIncompleteInstall": incomplete,
        "LaunchCommand": "",
        "LaunchExecutable": exe,
        "AppCategories": categories.split(',').collect::<Vec<_>>(),
        "DisplayName": display,
        "InstallLocation": location.to_string_lossy(),
        "CatalogNamespace": ns,
        "CatalogItemId": catalog,
        "AppName": app,
        "MainGameAppName": main,
        "bIsApplication": true,
        "bIsManaged": false,
    })
    .to_string()
}

/// Epic: Lantern Valley installed; Night Courier still installing; a piece of DLC for Lantern
/// Valley and an Unreal Engine install, neither of them a game; and a file that is not JSON.
pub(crate) fn epic(tag: &str) -> EpicFixture {
    let scratch = crate::test_temp::dir(tag);
    let manifests = scratch.join("Epic").join("EpicGamesLauncher").join("Data").join("Manifests");
    let lantern = scratch.join("Epic Games").join("LanternValley");
    let courier = scratch.join("Epic Games").join("NightCourier");
    write(lantern.join("Binaries").join("Win64").join("LanternValley.exe"), "x");
    std::fs::create_dir_all(&courier).unwrap();
    write(manifests.join("A1.item"), &item("Lantern Valley", &lantern, "Binaries/Win64/LanternValley.exe", "ns1", "item1", "Lantern", "Lantern", false, "public,games,applications"));
    write(manifests.join("B2.item"), &item("Night Courier", &courier, "Courier.exe", "ns2", "item2", "Courier", "Courier", true, "public,games,applications"));
    write(manifests.join("C3.item"), &item("Lantern Valley: Night Market", &lantern, "", "ns1", "dlc1", "LanternDLC", "Lantern", false, "public,addons"));
    write(manifests.join("D4.item"), &item("Unreal Engine", &scratch.join("UE_5.3"), "Engine/Binaries/Win64/UnrealEditor.exe", "ue", "ue1", "UE_5.3", "UE_5.3", false, "public,engines"));
    write(manifests.join("E5.item"), "{ this is not JSON");
    EpicFixture { _scratch: scratch, manifests, lantern }
}

/// GOG's installed games and their folders.
pub(crate) struct GogFixture {
    pub _scratch: crate::test_temp::TempPath,
    pub installed: Vec<Installed>,
    /// Old Forge's folder.
    pub forge: PathBuf,
}

/// GOG: Old Forge, whose .info lists its manual first and its primary play task (a program in
/// `bin` with a working directory and arguments) second; and Gone Valley, whose folder is not there.
pub(crate) fn gog(tag: &str) -> GogFixture {
    let scratch = crate::test_temp::dir(tag);
    let forge = scratch.join("GOG Games").join("Old Forge");
    write(forge.join("bin").join("forge.exe"), "x");
    let info = serde_json::json!({
        "buildId": "1",
        "clientId": "1",
        "gameId": "1207658924",
        "language": "English",
        "name": "Old Forge",
        "playTasks": [
            { "category": "document", "isPrimary": false, "name": "Manual", "path": "manual.pdf", "type": "FileTask" },
            { "category": "game", "isPrimary": true, "name": "Old Forge", "path": "bin\\forge.exe", "workingDir": "bin", "arguments": "-windowed -lang en", "type": "FileTask" }
        ],
        "rootGameId": "1207658924",
        "version": 1
    });
    write(forge.join("goggame-1207658924.info"), &info.to_string());
    let installed = vec![
        Installed { id: "1207658924".into(), folder: forge.clone(), name: "Old Forge".into() },
        Installed { id: "1400000001".into(), folder: scratch.join("GOG Games").join("Gone Valley"), name: "Gone Valley".into() },
    ];
    GogFixture { _scratch: scratch, installed, forge }
}

/// All three stores, kept alive together, and the `Sources` pointing at them.
pub(crate) struct AllFixture {
    pub steam: SteamFixture,
    pub epic: EpicFixture,
    pub gog: GogFixture,
}

impl AllFixture {
    pub fn sources(&self) -> Sources {
        Sources { steam_root: Some(self.steam.root.clone()), epic_manifests: Some(self.epic.manifests.clone()), gog_games: Some(self.gog.installed.clone()) }
    }
}

pub(crate) fn all(tag: &str) -> AllFixture {
    AllFixture { steam: steam(&format!("{tag}_steam")), epic: epic(&format!("{tag}_epic")), gog: gog(&format!("{tag}_gog")) }
}
