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
    /// `appcache\stats`, where the achievement records are.
    pub stats: PathBuf,
}

fn write(path: PathBuf, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn manifest(appid: &str, name: &str, flags: &str, installdir: &str) -> String {
    manifest_with(appid, name, flags, installdir, "\t\"SizeOnDisk\"\t\t\"1000\"\n")
}

/// An appmanifest with `extra` lines (each `\t"Key"\t\t"value"\n`) after its installdir.
fn manifest_with(appid: &str, name: &str, flags: &str, installdir: &str, extra: &str) -> String {
    format!("\"AppState\"\n{{\n\t\"appid\"\t\t\"{appid}\"\n\t\"universe\"\t\t\"1\"\n\t\"name\"\t\t\"{name}\"\n\t\"StateFlags\"\t\t\"{flags}\"\n\t\"installdir\"\t\t\"{installdir}\"\n{extra}}}\n")
}

/// The Steam account the fixture's records are kept for (its `userdata` folder's number).
pub(crate) const ACCOUNT: u32 = 1001;
/// Another account, which signed in later but is not the one marked MostRecent.
const OTHER_ACCOUNT: u32 = 2002;
/// A 64-bit Steam id's base: account id 0.
const STEAM_ID_BASE: u64 = 76_561_197_960_265_728;

/// A value in Valve's binary KeyValues (binary_keyvalues.rs), for writing fixture files.
pub(crate) enum B {
    Block(Vec<(&'static str, B)>),
    Str(&'static str),
    Int(i32),
}

/// `entries` written as a binary KeyValues file, as Steam writes its stats: each entry's type
/// byte, its name, its value; each block and the whole file ended by 0x08.
pub(crate) fn bkv(entries: &[(&'static str, B)]) -> Vec<u8> {
    fn put(out: &mut Vec<u8>, entries: &[(&'static str, B)]) {
        for (name, value) in entries {
            let kind = match value {
                B::Block(_) => 0u8,
                B::Str(_) => 1,
                B::Int(_) => 2,
            };
            out.push(kind);
            out.extend_from_slice(name.as_bytes());
            out.push(0);
            match value {
                B::Block(inner) => put(out, inner),
                B::Str(s) => {
                    out.extend_from_slice(s.as_bytes());
                    out.push(0);
                }
                B::Int(n) => out.extend_from_slice(&n.to_le_bytes()),
            }
        }
        out.push(8);
    }
    let mut out = Vec::new();
    put(&mut out, entries);
    out
}

/// One achievement in a schema: its bit's key, its id, its English name and description, and
/// whether it is hidden; `bit` when it carries its own.
fn achievement(key: &'static str, id: &'static str, name: &'static str, desc: &'static str, hidden: bool, bit: Option<i32>) -> (&'static str, B) {
    let display = B::Block(vec![
        ("name", B::Block(vec![("english", B::Str(name)), ("german", B::Str("Etwas")), ("token", B::Str("NEW_ACHIEVEMENT_NAME"))])),
        ("desc", B::Block(vec![("english", B::Str(desc)), ("german", B::Str("Etwas tun."))])),
        ("hidden", B::Int(hidden as i32)),
        ("icon", B::Str("0123456789abcdef.jpg")),
    ]);
    let mut entry = vec![("name", B::Str(id)), ("display", display)];
    if let Some(bit) = bit {
        entry.push(("bit", B::Int(bit)));
    }
    (key, B::Block(entry))
}

/// A screenshot picture: a small landscape (sky, sun and hills) in `tint`, as a JPEG.
pub(crate) fn screenshot_jpeg(path: &std::path::Path, tint: [u8; 3]) {
    picture(path, 480, 270, tint);
}

/// A logo: a light emblem (a ring and a bar) on a see-through ground, as a PNG.
fn logo_png(path: &std::path::Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let (w, h) = (320u32, 120u32);
    let img = image::RgbaImage::from_fn(w, h, |x, y| {
        let (fx, fy) = (x as f32, y as f32);
        let ring = ((fx - 60.0).powi(2) + (fy - 60.0).powi(2)).sqrt();
        let on = (34.0..48.0).contains(&ring) || ((120.0..300.0).contains(&fx) && (44.0..76.0).contains(&fy));
        if on { image::Rgba([250, 244, 230, 255]) } else { image::Rgba([0, 0, 0, 0]) }
    });
    img.save(path).unwrap();
}

/// A picture `w` by `h`: a landscape (sky, sun and hills) in `tint`, saved in the format its name
/// says.
pub(crate) fn picture(path: &std::path::Path, w: u32, h: u32, tint: [u8; 3]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let img = image::RgbImage::from_fn(w, h, |x, y| {
        let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
        let hill = 0.62 + 0.08 * (fx * 9.0 + tint[0] as f32 * 0.05).sin() + 0.04 * (fx * 23.0).sin();
        let sun = ((fx - 0.72).powi(2) + (fy - 0.3).powi(2)).sqrt() < 0.07;
        let c = if sun {
            [250.0, 230.0, 170.0]
        } else if fy > hill {
            let d = 1.0 - (fy - hill);
            [tint[0] as f32 * 0.35 * d, tint[1] as f32 * 0.55 * d, tint[2] as f32 * 0.3 * d]
        } else {
            let t = fy / hill;
            [tint[0] as f32 * (1.0 - t) + 240.0 * t, tint[1] as f32 * (1.0 - t) + 170.0 * t, tint[2] as f32 * (1.0 - t) + 120.0 * t]
        };
        image::Rgb([c[0].min(255.0) as u8, c[1].min(255.0) as u8, c[2].min(255.0) as u8])
    });
    img.save(path).unwrap();
}

/// Set a file's modification time (Unix seconds).
fn touch(path: &std::path::Path, unix: u64) {
    let file = std::fs::File::options().write(true).open(path).unwrap();
    file.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(unix)).unwrap();
}

/// Steam: Orbital Farmstead installed; Deep Rover still downloading (StateFlags 1026); Sunken
/// Archive installed but its folder gone; Steam's redistributables; and in the second library Old
/// Lighthouse, installed in "Lighthouse".
///
/// The person's records (2026-10-19), for the account [`ACCOUNT`] (marked MostRecent in
/// loginusers.vdf; another account signed in later): Orbital Farmstead played 754 minutes, last on
/// 21 September 2026 (its manifest says earlier: the person's record wins), its size, its last
/// update, five achievements of which three are unlocked, three screenshots (the two newest with
/// Steam's small copies) and three Workshop items; Old Lighthouse played 42 minutes, never
/// started since (no last-played time), no achievements, no screenshots and no Workshop.
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
    let farm_extra = "\t\"LastUpdated\"\t\t\"1791000000\"\n\t\"LastPlayed\"\t\t\"1789500000\"\n\t\"SizeOnDisk\"\t\t\"33471534235\"\n";
    write(apps.join("appmanifest_1000010.acf"), &manifest_with("1000010", "Orbital Farmstead", "4", "Orbital Farmstead", farm_extra));
    write(apps.join("common").join("Orbital Farmstead").join("farmstead.exe"), "x");
    write(apps.join("appmanifest_1000020.acf"), &manifest("1000020", "Deep Rover", "1026", "Deep Rover"));
    write(apps.join("appmanifest_1000040.acf"), &manifest("1000040", "Sunken Archive", "4", "Sunken Archive"));
    write(apps.join("appmanifest_228980.acf"), &manifest("228980", "Steamworks Common Redistributables", "4", "Steamworks Shared"));
    write(apps.join("common").join("Steamworks Shared").join("x.txt"), "x");
    let apps2 = second.join("steamapps");
    write(apps2.join("appmanifest_1000030.acf"), &manifest_with("1000030", "Old Lighthouse", "4", "Lighthouse", "\t\"SizeOnDisk\"\t\t\"2147483648\"\n"));
    write(apps2.join("common").join("Lighthouse").join("lighthouse.exe"), "x");

    // The accounts: ACCOUNT marked MostRecent, OTHER_ACCOUNT signed in later.
    let id64 = |a: u32| STEAM_ID_BASE + a as u64;
    write(
        root.join("config").join("loginusers.vdf"),
        &format!(
            "\"users\"\n{{\n\t\"{}\"\n\t{{\n\t\t\"AccountName\"\t\t\"farmer\"\n\t\t\"MostRecent\"\t\t\"1\"\n\t\t\"Timestamp\"\t\t\"100\"\n\t}}\n\t\"{}\"\n\t{{\n\t\t\"AccountName\"\t\t\"visitor\"\n\t\t\"MostRecent\"\t\t\"0\"\n\t\t\"Timestamp\"\t\t\"200\"\n\t}}\n}}\n",
            id64(ACCOUNT),
            id64(OTHER_ACCOUNT)
        ),
    );
    let user = root.join("userdata").join(ACCOUNT.to_string());
    std::fs::create_dir_all(root.join("userdata").join(OTHER_ACCOUNT.to_string()).join("config")).unwrap();
    write(
        user.join("config").join("localconfig.vdf"),
        "\"UserLocalConfigStore\"\n{\n\t\"SharedAuth\"\n\t{\n\t\t\"id\"\t\t\"1\"\n\t\t\"AuthData\"\t\t\"not for the page\"\n\t}\n\t\"Software\"\n\t{\n\t\t\"Valve\"\n\t\t{\n\t\t\t\"Steam\"\n\t\t\t{\n\t\t\t\t\"apps\"\n\t\t\t\t{\n\t\t\t\t\t\"1000010\"\n\t\t\t\t\t{\n\t\t\t\t\t\t\"LastPlayed\"\t\t\"1790000000\"\n\t\t\t\t\t\t\"Playtime\"\t\t\"754\"\n\t\t\t\t\t\t\"cloud\"\n\t\t\t\t\t\t{\n\t\t\t\t\t\t\t\"last_sync_state\"\t\t\"synchronized\"\n\t\t\t\t\t\t}\n\t\t\t\t\t}\n\t\t\t\t\t\"1000030\"\n\t\t\t\t\t{\n\t\t\t\t\t\t\"LastPlayed\"\t\t\"0\"\n\t\t\t\t\t\t\"Playtime\"\t\t\"42\"\n\t\t\t\t\t}\n\t\t\t\t\t\"1000020\"\n\t\t\t\t\t{\n\t\t\t\t\t\t\"cloud\"\n\t\t\t\t\t\t{\n\t\t\t\t\t\t}\n\t\t\t\t\t}\n\t\t\t\t}\n\t\t\t}\n\t\t}\n\t}\n}\n",
    );

    // Orbital Farmstead's screenshots: three, a day apart, the two newest with Steam's small copies.
    let shots = user.join("760").join("remote").join("1000010").join("screenshots");
    for (i, (name, tint)) in [("20261001120000_1.jpg", [70, 120, 200]), ("20261002120000_1.jpg", [200, 110, 60]), ("20261003120000_1.jpg", [60, 170, 120])].into_iter().enumerate() {
        screenshot_jpeg(&shots.join(name), tint);
        touch(&shots.join(name), 1_790_000_000 + i as u64 * 86_400);
        if i > 0 {
            screenshot_jpeg(&shots.join("thumbnails").join(name), tint);
        }
    }

    // Its achievements: five in two blocks (the second numbered by its own `bit`), and a stat
    // that is not achievements; three unlocked (Long Night with no time recorded).
    let stats = root.join("appcache").join("stats");
    let schema = bkv(&[(
        "1000010",
        B::Block(vec![
            (
                "stats",
                B::Block(vec![
                    (
                        "1",
                        B::Block(vec![
                            (
                                "bits",
                                B::Block(vec![
                                    achievement("0", "SEEDLING", "Seedling", "Plant your first seed.", false, None),
                                    achievement("1", "DEEP_ROOTS", "Deep Roots", "Grow a tree for a hundred days.", false, None),
                                    achievement("2", "LONG_NIGHT", "Long Night", "Keep the greenhouse warm through a winter night.", false, None),
                                    achievement("3", "SECRET_GARDEN", "Secret Garden", "Find the garden behind the waterfall.", true, None),
                                ]),
                            ),
                            ("type", B::Str("ACHIEVEMENTS")),
                        ]),
                    ),
                    ("2", B::Block(vec![("bits", B::Block(vec![achievement("7", "FIRST_HARVEST", "First Harvest", "Bring in your first crop.", false, Some(0))])), ("type", B::Str("4")), ("type_int", B::Int(4))])),
                    ("3", B::Block(vec![("type", B::Str("INT")), ("name", B::Str("Crops")), ("type_int", B::Int(1))])),
                ]),
            ),
            ("version", B::Int(3)),
            ("gamename", B::Str("Orbital Farmstead")),
        ]),
    )]);
    std::fs::create_dir_all(&stats).unwrap();
    std::fs::write(stats.join("UserGameStatsSchema_1000010.bin"), schema).unwrap();
    let record = bkv(&[(
        "cache",
        B::Block(vec![
            ("crc", B::Int(-12345)),
            ("PendingChanges", B::Int(0)),
            ("1", B::Block(vec![("data", B::Int(0b101)), ("AchievementTimes", B::Block(vec![("0", B::Int(1_790_000_000))]))])),
            ("2", B::Block(vec![("data", B::Int(1)), ("AchievementTimes", B::Block(vec![("0", B::Int(1_789_000_000))]))])),
            ("3", B::Block(vec![("data", B::Int(42))])),
        ]),
    )]);
    std::fs::write(stats.join(format!("UserGameStats_{ACCOUNT}_1000010.bin")), record).unwrap();

    // Its Workshop: two items recorded, and a third only on the disk (and a folder that is not an item).
    let workshop = apps.join("workshop");
    write(
        workshop.join("appworkshop_1000010.acf"),
        "\"AppWorkshop\"\n{\n\t\"appid\"\t\t\"1000010\"\n\t\"SizeOnDisk\"\t\t\"1502500000\"\n\t\"WorkshopItemsInstalled\"\n\t{\n\t\t\"3000001\"\n\t\t{\n\t\t\t\"size\"\t\t\"1500000000\"\n\t\t\t\"timeupdated\"\t\t\"1780000000\"\n\t\t\t\"manifest\"\t\t\"1\"\n\t\t}\n\t\t\"3000002\"\n\t\t{\n\t\t\t\"size\"\t\t\"2500000\"\n\t\t\t\"timeupdated\"\t\t\"1785000000\"\n\t\t\t\"manifest\"\t\t\"2\"\n\t\t}\n\t}\n\t\"WorkshopItemDetails\"\n\t{\n\t}\n}\n",
    );
    for id in ["3000001", "3000002", "3000003", "notes"] {
        write(workshop.join("content").join("1000010").join(id).join("mod.txt"), "x");
    }

    // The pictures Steam's client cached: Orbital Farmstead's directly in its folder, Old
    // Lighthouse's one per hash-named folder (no header.jpg), Deep Rover's in the old flat layout,
    // Sunken Archive none.
    let cache = root.join("appcache").join("librarycache");
    let farm = cache.join("1000010");
    picture(&farm.join("header.jpg"), 460, 215, [70, 140, 90]);
    picture(&farm.join("library_hero.jpg"), 960, 310, [60, 120, 170]);
    picture(&farm.join("library_600x900.jpg"), 300, 450, [70, 140, 90]);
    logo_png(&farm.join("logo.png"));
    let light = cache.join("1000030");
    picture(&light.join("2012c0098b08cdff7665a6a808e218b1ab1c6399").join("library_capsule.jpg"), 300, 450, [40, 60, 130]);
    picture(&light.join("84d0df065c36eb8ded7bdf0d8fccd19d43b6c005").join("library_header.jpg"), 460, 215, [40, 60, 130]);
    picture(&light.join("a06bedfeefe046496d0155b46cff309933e7d46b").join("library_hero.jpg"), 960, 310, [30, 50, 110]);
    picture(&light.join("fccb20f309ac5ae05b3cb4576a8a3357edf86f2e.jpg"), 32, 32, [30, 50, 110]);
    picture(&cache.join("1000020_header.jpg"), 460, 215, [150, 90, 50]);
    SteamFixture { _scratch: scratch, root, second_library: second, stats }
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
        "InstallSize": 8_589_934_592u64,
        "AppVersionString": "2.4.1-CL-118",
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
    // The launcher's catalog beside the manifests (epic_covers.rs): Lantern Valley's covers'
    // addresses, and Fortnite's.
    let catalog = String::from_utf8(super::epic_covers::tests::catalog_bytes()).unwrap();
    write(scratch.join("Epic").join("EpicGamesLauncher").join("Data").join("Catalog").join("catcache.bin"), &catalog);
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
        Sources { steam_root: Some(self.steam.root.clone()), epic_manifests: Some(self.epic.manifests.clone()), gog_games: Some(self.gog.installed.clone()), steam_active_user: None }
    }
}

pub(crate) fn all(tag: &str) -> AllFixture {
    AllFixture { steam: steam(&format!("{tag}_steam")), epic: epic(&format!("{tag}_epic")), gog: gog(&format!("{tag}_gog")) }
}
