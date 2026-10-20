//! THIS PERSON'S MICROSOFT STORE APPS (2026-10-20), read ONLY. A program installed from the
//! Microsoft Store as a package (MSIX) is not in Windows' installed-programs list that records.rs
//! reads: Windows lists such packages through a separate interface, `PackageManager` (the dated
//! finding docs/reference/findings/2026-10-19-tools-page-install-launch-and-icons.md, 1.7 and
//! judgement call 1). Its `FindPackagesForUser` with an empty user "retrieves the packages for the
//! current user", and needs administrator rights only for ANOTHER user's; in the Windows crate's
//! naming that is `FindPackagesByUserSecurityIdPackageFamilyName`, the same call narrowed to one
//! package family.
//!
//! WHAT IS READ, and no more: only the package families the catalog names (each program's
//! `store_family`, data/external/catalog.json), never the whole list; of each package found, its
//! name, its version, its folder and the app ids of its apps (each `<family>!<app>`, what Windows
//! starts it by). Nothing is installed, removed or changed: `PackageManager` is asked only to find.
//! Read on the Apps page's worker thread when the page opens and on Refresh, never every frame, and
//! never sent anywhere.
//!
//! STARTING ONE: a Store app is not started by its program file (Windows does not support that for
//! a packaged app) but by its app id, as the Start menu starts it: `shell:AppsFolder\<app id>`,
//! through the one launcher (games/launch.rs, `Launch::StoreApp`). [`app_id_ok`] is the check every
//! app id passes first, so nothing but an app id is ever handed to Explorer.
//!
//! The Windows crate features this needs (Cargo.toml): `ApplicationModel` (the package),
//! `ApplicationModel_Core` (its apps' list) and `Management_Deployment` (`PackageManager`).

use super::records::StorePackage;

/// Whether `family` is shaped like a package family name: its name (letters, digits, `.` and
/// `-`), `_`, and its publisher's 13-letter id (lower-case letters and digits):
/// "BlenderFoundation.Blender_ppwjx1n5r4v9t".
pub fn family_ok(family: &str) -> bool {
    let Some((name, publisher)) = family.rsplit_once('_') else { return false };
    (3..=50).contains(&name.len())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
        && publisher.len() == 13
        && publisher.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

/// Whether `id` is shaped like a Store app's app id, `<package family name>!<app>`, the app's name
/// being letters, digits and `.`, beginning with a letter ("BlenderFoundation.Blender_
/// ppwjx1n5r4v9t!Blender"). Nothing else is ever handed to Explorer as an app to start.
pub fn app_id_ok(id: &str) -> bool {
    let Some((family, app)) = id.split_once('!') else { return false };
    family_ok(family)
        && (1..=64).contains(&app.len())
        && app.starts_with(|c: char| c.is_ascii_alphabetic())
        && app.chars().all(|c| c.is_ascii_alphanumeric() || c == '.')
}

/// The Store apps of `families` installed for the person using this computer (Windows only;
/// elsewhere none). A family not well formed is not asked about.
pub fn this_computer(families: &[String]) -> Vec<StorePackage> {
    #[cfg(windows)]
    {
        windows_read::read(families)
    }
    #[cfg(not(windows))]
    {
        let _ = families;
        Vec::new()
    }
}

#[cfg(windows)]
mod windows_read {
    use super::{app_id_ok, family_ok, StorePackage};
    use std::path::PathBuf;
    use windows::core::HSTRING;
    use windows::Management::Deployment::PackageManager;

    /// At most this many families are asked about, and this many packages kept of each.
    const MOST_FAMILIES: usize = 500;
    const MOST_PACKAGES: usize = 8;

    pub(super) fn read(families: &[String]) -> Vec<StorePackage> {
        let mut asked: Vec<&str> = families.iter().map(String::as_str).filter(|f| family_ok(f)).collect();
        asked.sort_unstable();
        asked.dedup();
        if asked.is_empty() {
            return Vec::new();
        }
        crate::games::note_disk_read();
        let Ok(manager) = PackageManager::new() else { return Vec::new() };
        // An empty user is the person using this computer (no administrator rights needed).
        let me = HSTRING::new();
        let mut out = Vec::new();
        for family in asked.into_iter().take(MOST_FAMILIES) {
            let Ok(found) = manager.FindPackagesByUserSecurityIdPackageFamilyName(&me, &HSTRING::from(family)) else { continue };
            for package in found.into_iter().take(MOST_PACKAGES) {
                // A framework or a resource package has no app of its own.
                if package.IsFramework().unwrap_or(false) || package.IsResourcePackage().unwrap_or(false) {
                    continue;
                }
                let version = package.Id().and_then(|id| id.Version()).ok().map(|v| format!("{}.{}.{}.{}", v.Major, v.Minor, v.Build, v.Revision));
                let name = package.DisplayName().ok().map(|n| n.to_string_lossy().trim().to_string()).filter(|n| !n.is_empty());
                let folder = package.InstalledPath().ok().map(|p| p.to_string_lossy()).filter(|p| !p.is_empty()).map(PathBuf::from);
                let mut apps = Vec::new();
                if let Ok(entries) = package.GetAppListEntries() {
                    for entry in entries.into_iter().take(16) {
                        // AppUserModelId is the newer property; AppInfo's is the same id on older Windows.
                        let id = entry.AppUserModelId().or_else(|_| entry.AppInfo().and_then(|i| i.AppUserModelId())).map(|s| s.to_string_lossy());
                        if let Ok(id) = id {
                            if app_id_ok(&id) && id.starts_with(family) && !apps.contains(&id) {
                                apps.push(id);
                            }
                        }
                    }
                }
                out.push(StorePackage { family: family.to_string(), name, version, folder, apps });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This person's Store apps of a family nearly every Windows 10 and 11 has (Calculator) and of
    /// the catalog's, printed for a look by hand: `node scripts/lib-tests.js
    /// software::store::tests::print --ignored --nocapture`. Ignored, because what it finds depends
    /// on the machine. Like the page, it only asks Windows to find; it starts nothing.
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn print_this_persons_store_apps() {
        let catalog = super::super::catalog::parse(&std::fs::read_to_string("data/external/catalog.json").unwrap()).unwrap();
        let mut families: Vec<String> = catalog.iter().flat_map(|e| e.store_family.iter().cloned()).collect();
        families.push("Microsoft.WindowsCalculator_8wekyb3d8bbwe".into());
        let found = this_computer(&families);
        println!("{} families asked about, {} packages found:", families.len(), found.len());
        for p in &found {
            println!("  {} | {:?} | {:?} | {:?} | {:?}", p.family, p.name, p.version, p.folder, p.apps);
        }
    }

    /// WHAT IS HANDED TO EXPLORER IS ONLY EVER A STORE APP'S ID: a package family name is its name,
    /// `_` and a 13-letter publisher id; an app id is that, `!` and the app's own name; anything
    /// else (a path, a switch, a command, a second `!`, spaces, a publisher id of another length or
    /// in capitals) is refused.
    ///
    /// Seen red 2026-10-20 with `app_id_ok` checking only that a `!` is there (`split_once`, then
    /// `true`): an id with nothing after its `!` was taken, the message being
    /// `"BlenderFoundation.Blender_ppwjx1n5r4v9t!" is not an app id (a command after the app id, or
    /// none)`.
    #[test]
    fn only_a_store_apps_id_is_handed_to_explorer() {
        for ok in ["BlenderFoundation.Blender_ppwjx1n5r4v9t", "GIMP.43237F745459_nq49gba4h4mx8", "VisualComputingLab-ISTI-C.MeshLab_c8c2mafe4x7s8", "dotPDNLLC.paint.net_h55e3w7q8jbva"] {
            assert!(family_ok(ok), "{ok} is a family name");
        }
        for bad in ["", "Blender", "Blender_ppwjx1n5r4v9", "Blender_PPWJX1N5R4V9T", "Blen der_ppwjx1n5r4v9t", "a_ppwjx1n5r4v9t", "C:\\x_ppwjx1n5r4v9t"] {
            assert!(!family_ok(bad), "{bad:?} is not a family name");
        }
        assert!(app_id_ok("BlenderFoundation.Blender_ppwjx1n5r4v9t!Blender"));
        assert!(app_id_ok("19568ShareX.ShareX_egrzcvs15399j!ShareX"));
        for bad in [
            "BlenderFoundation.Blender_ppwjx1n5r4v9t",
            "BlenderFoundation.Blender_ppwjx1n5r4v9t!",
            "BlenderFoundation.Blender_ppwjx1n5r4v9t!Blender & calc",
            "BlenderFoundation.Blender_ppwjx1n5r4v9t!Blender!Two",
            "BlenderFoundation.Blender_ppwjx1n5r4v9t!1Blender",
            "BlenderFoundation.Blender_ppwjx1n5r4v9t!..\\..\\cmd",
            "C:\\Windows\\System32\\cmd.exe",
            "--help!App",
        ] {
            assert!(!app_id_ok(bad), "{bad:?} is not an app id (a command after the app id, or none)");
        }
    }
}
