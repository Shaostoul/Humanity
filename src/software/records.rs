//! What Windows records about the programs installed on this computer, read ONLY (every key is
//! opened with KEY_READ through games/registry.rs; nothing here can write a key or a value):
//!
//! - THE INSTALLED-PROGRAMS LIST, the one Windows' own "Installed apps" shows: one Uninstall key per
//!   program, under `SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall` in HKLM (programs for
//!   everyone) and HKCU (programs for this person), each in both the 64-bit and the 32-bit view of
//!   the registry (a 32-bit installer writes under WOW6432Node). Of each: DisplayName,
//!   DisplayVersion, InstallLocation and DisplayIcon. A key marked SystemComponent is left out, as
//!   Windows' own list leaves it out.
//! - APP PATHS (`SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\<program>.exe`), where an
//!   installer tells Windows where its program is so that Run and the Start menu can find it by its
//!   file name.
//! - THE START MENU'S SHORTCUTS (2026-10-19, shortcuts.rs), by name only: a shortcut is opened
//!   only when a program's record gives no program file (software/matcher.rs).
//! - THIS PERSON'S MICROSOFT STORE APPS (2026-10-20, store.rs), only of the package families the
//!   catalog names, through Windows' `PackageManager` (asked only to find).
//!
//! Read on the Tools page's worker thread when the page opens and on Refresh, never every frame.
//! The page's tests never read the real registry: they hand the matcher a [`Records`] of their
//! own, which is what [`Records`] being a plain struct is for.

use std::path::PathBuf;

/// Where in the registry a record was read (said by the this-computer printout).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Hive {
    /// HKLM, the 64-bit view: programs installed for everyone.
    #[default]
    Machine,
    /// HKLM, the 32-bit view (WOW6432Node).
    Machine32,
    /// HKCU: programs installed for this person only.
    User,
}

/// One program in Windows' installed-programs list.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Uninstall {
    /// The key's own name (a product code, or the program's name).
    pub key: String,
    /// The name Windows shows for it ("GIMP 3.2.4").
    pub display_name: String,
    /// Its version as its installer wrote it.
    pub version: Option<String>,
    /// The folder its installer says it is in (many leave this empty).
    pub location: Option<PathBuf>,
    /// DisplayIcon as written: often the program itself, "C:\...\gimp-3.exe" or "...\obs64.exe,0".
    pub icon: Option<String>,
    pub hive: Hive,
}

/// A program Windows can find by its file name (App Paths).
#[derive(Debug, Clone, PartialEq)]
pub struct AppPath {
    /// Its file name, as the key is named ("notepad++.exe").
    pub exe_name: String,
    /// Where it is.
    pub path: PathBuf,
}

/// One of this person's Microsoft Store apps (2026-10-20, store.rs): a package of a family the
/// catalog names, as Windows' `PackageManager` lists it for the person using this computer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StorePackage {
    /// Its package family name ("BlenderFoundation.Blender_ppwjx1n5r4v9t").
    pub family: String,
    /// The name it shows, when it says one.
    pub name: Option<String>,
    /// Its version ("5.2.2.0").
    pub version: Option<String>,
    /// The folder Windows installed it in.
    pub folder: Option<PathBuf>,
    /// Its apps' ids, `<family>!<app>`, the first its main one: what Windows starts it by.
    pub apps: Vec<String>,
}

/// What Windows records, read once.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Records {
    pub uninstall: Vec<Uninstall>,
    pub app_paths: Vec<AppPath>,
    /// The Start menu's shortcuts, by name (shortcuts.rs).
    pub shortcuts: Vec<super::shortcuts::Shortcut>,
    /// This person's Microsoft Store apps of the families the catalog names (store.rs).
    pub store: Vec<StorePackage>,
}

/// A path as an installer wrote it: quotes taken off, a trailing ",0" (an icon's index) taken off,
/// and `%Name%` replaced by the environment variable of that name. `None` when nothing is left.
pub fn clean_path(written: &str) -> Option<PathBuf> {
    let mut s = written.trim();
    // "C:\x\a.exe",0 or C:\x\a.exe,0: the icon's index after the last comma.
    if let Some(at) = s.rfind(',') {
        if s[at + 1..].trim().trim_start_matches('-').chars().all(|c| c.is_ascii_digit()) && !s[at + 1..].trim().is_empty() {
            s = s[..at].trim();
        }
    }
    let s = s.trim_matches('"').trim();
    if s.is_empty() {
        return None;
    }
    Some(PathBuf::from(expand_env(s)))
}

/// `%Name%` replaced by the environment variable `Name` (left as written when there is none).
fn expand_env(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) if end > 0 => {
                let name = &after[..end];
                match std::env::var(name) {
                    Ok(v) => out.push_str(&v),
                    Err(_) => {
                        out.push('%');
                        out.push_str(name);
                        out.push('%');
                    }
                }
                rest = &after[end + 1..];
            }
            _ => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// This computer's records (Windows only; elsewhere none), with this person's Store apps of
/// `store_families` (the catalog's `store_family` fields).
pub fn this_computer(store_families: &[String]) -> Records {
    #[cfg(windows)]
    {
        let mut records = windows_read::read();
        records.shortcuts = super::shortcuts::list(&super::shortcuts::start_menu_folders());
        records.store = super::store::this_computer(store_families);
        records
    }
    #[cfg(not(windows))]
    {
        let _ = store_families;
        Records::default()
    }
}

#[cfg(windows)]
mod windows_read {
    use super::{clean_path, AppPath, Hive, Records, Uninstall};
    use crate::games::registry::{self, Root, View};

    const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
    const APP_PATHS: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths";
    /// The four places each is read from. HKCU's software is one tree in both views on current
    /// Windows, so its second read finds the same keys again; they are kept once.
    const PLACES: [(Root, View, Hive); 4] = [
        (Root::LocalMachine, View::Bits64, Hive::Machine),
        (Root::LocalMachine, View::Bits32, Hive::Machine32),
        (Root::CurrentUser, View::Bits64, Hive::User),
        (Root::CurrentUser, View::Bits32, Hive::User),
    ];
    /// At most this many keys are read from one place.
    const MOST: usize = 5_000;

    pub(super) fn read() -> Records {
        crate::games::note_disk_read();
        let mut out = Records::default();
        for (root, view, hive) in PLACES {
            let Some(list) = registry::open_view(root, UNINSTALL, view) else { continue };
            for name in list.subkeys().into_iter().take(MOST) {
                let Some(key) = list.open(&name) else { continue };
                let Some(display_name) = key.string("DisplayName").map(|n| n.trim().to_string()).filter(|n| !n.is_empty()) else { continue };
                if key.dword("SystemComponent") == Some(1) {
                    continue;
                }
                if out.uninstall.iter().any(|u| u.hive == hive && u.key.eq_ignore_ascii_case(&name) && u.display_name == display_name) {
                    continue;
                }
                let text = |v: &str| key.string(v).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
                out.uninstall.push(Uninstall {
                    key: name.clone(),
                    display_name,
                    version: text("DisplayVersion"),
                    location: text("InstallLocation").and_then(|l| clean_path(&l)),
                    icon: text("DisplayIcon"),
                    hive,
                });
            }
            let Some(paths) = registry::open_view(root, APP_PATHS, view) else { continue };
            for name in paths.subkeys().into_iter().take(MOST) {
                if out.app_paths.iter().any(|a| a.exe_name.eq_ignore_ascii_case(&name)) {
                    continue;
                }
                let Some(key) = paths.open(&name) else { continue };
                // The key's own (unnamed) value is the program's path.
                let Some(path) = key.string("").and_then(|p| clean_path(&p)) else { continue };
                out.app_paths.push(AppPath { exe_name: name, path });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PATH AS AN INSTALLER WROTE IT: quotes and an icon's index taken off, `%SystemRoot%`
    /// filled in; a comma that is part of a folder's name kept; an empty one is nothing.
    ///
    /// Seen red 2026-10-19 with the index left on (the `rfind(',')` step taken out): "the icon's
    /// index taken off" got "C:\\Apps\\obs-studio\\bin\\64bit\\obs64.exe,0".
    #[test]
    fn a_path_as_an_installer_wrote_it() {
        assert_eq!(clean_path(r#""C:\Apps\GIMP 3\bin\gimp-3.exe""#), Some(PathBuf::from(r"C:\Apps\GIMP 3\bin\gimp-3.exe")));
        assert_eq!(clean_path(r"C:\Apps\obs-studio\bin\64bit\obs64.exe,0"), Some(PathBuf::from(r"C:\Apps\obs-studio\bin\64bit\obs64.exe")), "the icon's index taken off");
        assert_eq!(clean_path(r#""C:\Apps\x.exe",-101"#), Some(PathBuf::from(r"C:\Apps\x.exe")));
        assert_eq!(clean_path(r"C:\Tools, Old\a.exe"), Some(PathBuf::from(r"C:\Tools, Old\a.exe")), "a comma in a folder's name");
        assert_eq!(clean_path("  \"\" "), None);
        if let Ok(root) = std::env::var("SystemRoot") {
            assert_eq!(clean_path(r"%SystemRoot%\explorer.exe"), Some(PathBuf::from(format!(r"{root}\explorer.exe"))));
        }
        assert_eq!(clean_path(r"%NoSuchVariableHumanityOS%\a.exe"), Some(PathBuf::from(r"%NoSuchVariableHumanityOS%\a.exe")), "an unknown variable left as written");
    }
}
