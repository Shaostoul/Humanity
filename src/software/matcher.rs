//! Which catalog programs are installed: each entry's `detect` and `steam_appid` (data, in
//! data/external/catalog.json) matched against what Windows and Steam record (records.rs, and the
//! Games page's Steam reader). Runs on the Tools page's worker thread; the only disk work here is
//! asking whether a file is there, and reading an installed program's icon from its own file
//! (games/art.rs, the program opened as data, never run).
//!
//! An entry is installed when, in this order:
//! 1. a program in Windows' installed-programs list has a name its `display_name` pattern matches
//!    (of several, the one whose program is found on the disk, then the newest version);
//! 2. else Windows' App Paths knows one of its programs by file name;
//! 3. else Steam has its `steam_appid` fully installed.
//!
//! Its program (what Launch starts) is looked for in its install folder (each `exe` path), then
//! as its DisplayIcon (when that names one of its programs; with no `exe` in the data, any program
//! DisplayIcon names that is not an uninstaller), then in App Paths. A program that cannot be
//! found is still installed, just not startable from here.

use std::path::{Path, PathBuf};

use super::catalog::ToolEntry;
use super::records::{clean_path, Records, Uninstall};
use crate::games::art::Icon;
use crate::games::{Game, Launch, State};

/// Where an installed program was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    /// Windows' installed-programs list.
    Windows,
    /// Windows' App Paths.
    AppPaths,
    /// Steam's install records.
    Steam,
}

impl Via {
    /// Said in its details.
    pub fn words(self) -> &'static str {
        match self {
            Via::Windows => "Windows' list of installed programs",
            Via::AppPaths => "Windows' list of programs it can start by name (App Paths)",
            Via::Steam => "Steam",
        }
    }
}

/// A catalog program found on this computer.
#[derive(Debug, Clone, PartialEq)]
pub struct Installed {
    pub via: Via,
    /// The name Windows keeps for it, when found in its list.
    pub name_in_windows: Option<String>,
    /// Its version, as its installer wrote it.
    pub version: Option<String>,
    /// The folder it is installed in, when known.
    pub folder: Option<PathBuf>,
    /// Its program file, when found on the disk.
    pub program: Option<PathBuf>,
    /// What Launch does: its program, or Steam's own link for a copy Steam installed.
    pub launch: Option<Launch>,
    /// Its program's icon, read from the program's own file.
    pub icon: Option<Icon>,
}

impl Installed {
    /// What Show in folder shows: its program in its folder, else its folder.
    pub fn show(&self) -> Option<PathBuf> {
        self.program.clone().or_else(|| self.folder.clone())
    }
}

/// A file is there (counted as a disk read, for the page's no-reading-per-frame test).
fn is_file(p: &Path) -> bool {
    crate::games::note_disk_read();
    p.is_file()
}

/// The last part of a path written with either slash ("bin/krita.exe" is "krita.exe").
fn file_name(rel: &str) -> &str {
    rel.rsplit(['/', '\\']).next().unwrap_or(rel)
}

/// `rel` ("bin/krita.exe") under `folder`, with the separators Windows uses.
fn under(folder: &Path, rel: &str) -> PathBuf {
    let mut p = folder.to_path_buf();
    for part in rel.split(['/', '\\']).filter(|s| !s.is_empty() && *s != "." && *s != "..") {
        p.push(part);
    }
    p
}

/// Whether a program's file name looks like an uninstaller or a setup program (not the program).
fn looks_like_installer(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["unins", "uninst", "uninstall", "setup", "install", "update", "msiexec"].iter().any(|p| n.starts_with(p))
}

/// A version written as numbers and dots, as numbers to compare ("3.2.4.0" > "2.10.38").
fn version_key(v: Option<&str>) -> Vec<u64> {
    v.unwrap_or("").split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).map(|s| s.parse().unwrap_or(0)).collect()
}

/// The program of an installed-programs record, found on the disk: an `exe` under its install
/// folder, its DisplayIcon, or App Paths. `None` when none of them is there.
fn program_of(record: &Uninstall, exes: &[String], records: &Records) -> Option<PathBuf> {
    if let Some(folder) = &record.location {
        for rel in exes {
            let p = under(folder, rel);
            if is_file(&p) {
                return Some(p);
            }
        }
    }
    if let Some(icon) = record.icon.as_deref().and_then(clean_path) {
        let name = icon.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let is_exe = name.to_ascii_lowercase().ends_with(".exe");
        let named = if exes.is_empty() { is_exe && !looks_like_installer(&name) } else { exes.iter().any(|e| file_name(e).eq_ignore_ascii_case(&name)) };
        if named && is_file(&icon) {
            return Some(icon);
        }
    }
    app_path(exes, records)
}

/// One of `exes` in App Paths, found on the disk.
fn app_path(exes: &[String], records: &Records) -> Option<PathBuf> {
    for rel in exes {
        let name = file_name(rel);
        if let Some(a) = records.app_paths.iter().find(|a| a.exe_name.eq_ignore_ascii_case(name)) {
            if is_file(&a.path) {
                return Some(a.path.clone());
            }
        }
    }
    None
}

/// The install folder a program found on its own (by DisplayIcon or App Paths) is in: its path
/// with the entry's `exe` path taken off the end ("C:\Apps\obs-studio\bin\64bit\obs64.exe" with
/// "bin/64bit/obs64.exe" is "C:\Apps\obs-studio"), else the folder it is in.
fn folder_of(program: &Path, exes: &[String]) -> Option<PathBuf> {
    let parts = |p: &Path| p.components().map(|c| c.as_os_str().to_string_lossy().to_lowercase()).collect::<Vec<_>>();
    let have = parts(program);
    for rel in exes {
        let want: Vec<String> = rel.split(['/', '\\']).filter(|s| !s.is_empty()).map(str::to_lowercase).collect();
        if !want.is_empty() && want.len() < have.len() && have[have.len() - want.len()..] == want[..] {
            return program.ancestors().nth(want.len()).map(Path::to_path_buf);
        }
    }
    program.parent().map(Path::to_path_buf)
}

/// Start `program` in its own folder.
fn launch_of(program: &Path) -> Launch {
    Launch::Program { exe: program.to_path_buf(), args: String::new(), dir: program.parent().map(Path::to_path_buf).unwrap_or_default() }
}

/// Whether `entry` is installed, by `records` and Steam's installed games (`steam`), and how.
/// The icon is not read here (see [`find_with_icon`]).
pub fn find(entry: &ToolEntry, records: &Records, steam: &[Game]) -> Option<Installed> {
    let detect = entry.detect.clone().unwrap_or_default();
    // 1. Windows' installed-programs list.
    if let Some(pattern) = detect.display_name.as_deref().filter(|p| !p.trim().is_empty()) {
        // A pattern that does not compile matches nothing (the catalog test checks every one).
        if let Ok(re) = regex::RegexBuilder::new(pattern).case_insensitive(true).size_limit(1 << 20).build() {
            let mut best: Option<(bool, Vec<u64>, Installed)> = None;
            for record in records.uninstall.iter().filter(|u| re.is_match(&u.display_name)) {
                let program = program_of(record, &detect.exe, records);
                let folder = record.location.clone().or_else(|| program.as_deref().and_then(|p| folder_of(p, &detect.exe)));
                let found = Installed {
                    via: Via::Windows,
                    name_in_windows: Some(record.display_name.clone()),
                    version: record.version.clone(),
                    launch: program.as_deref().map(launch_of),
                    folder,
                    program,
                    icon: None,
                };
                let rank = (found.program.is_some(), version_key(record.version.as_deref()));
                if best.as_ref().is_none_or(|(p, v, _)| rank > (*p, v.clone())) {
                    best = Some((rank.0, rank.1, found));
                }
            }
            if let Some((_, _, found)) = best {
                return Some(found);
            }
        }
    }
    // 2. App Paths.
    if let Some(program) = app_path(&detect.exe, records) {
        return Some(Installed {
            via: Via::AppPaths,
            name_in_windows: None,
            version: None,
            folder: folder_of(&program, &detect.exe),
            launch: Some(launch_of(&program)),
            program: Some(program),
            icon: None,
        });
    }
    // 3. Steam.
    let appid = entry.steam_appid?.to_string();
    let game = steam.iter().find(|g| g.id == appid && g.state == State::Ready)?;
    let program = detect.exe.iter().map(|rel| under(&game.folder, rel)).find(|p| is_file(p));
    Some(Installed {
        via: Via::Steam,
        name_in_windows: None,
        version: None,
        folder: Some(game.folder.clone()),
        program,
        launch: game.play.clone(),
        icon: None,
    })
}

/// [`find`], and the icon of the program found, read from its own file.
pub fn find_with_icon(entry: &ToolEntry, records: &Records, steam: &[Game]) -> Option<Installed> {
    let mut found = find(entry, records, steam)?;
    found.icon = found.program.as_deref().and_then(crate::games::art::program_icon);
    Some(found)
}

#[cfg(test)]
mod tests {
    use super::super::catalog::Detect;
    use super::super::records::{AppPath, Hive};
    use super::*;

    fn entry(name: &str, display_name: Option<&str>, exe: &[&str], steam: Option<u32>) -> ToolEntry {
        ToolEntry {
            name: name.into(),
            kind: "software".into(),
            detect: Some(Detect { display_name: display_name.map(str::to_string), exe: exe.iter().map(|s| s.to_string()).collect() }),
            steam_appid: steam,
            ..ToolEntry::default()
        }
    }

    fn record(name: &str, version: Option<&str>, location: Option<&Path>, icon: Option<&str>) -> Uninstall {
        Uninstall { key: name.into(), display_name: name.into(), version: version.map(str::to_string), location: location.map(Path::to_path_buf), icon: icon.map(str::to_string), hive: Hive::Machine }
    }

    /// A program file in a scratch folder.
    fn program(dir: &Path, rel: &str) -> PathBuf {
        let p = under(dir, rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, "x").unwrap();
        p
    }

    /// THE MATCHER, AGAINST WINDOWS' RECORDS AS THE OPERATOR'S COMPUTER HAS THEM (2026-10-19): GIMP
    /// 2 and GIMP 3 both installed, GIMP 3 chosen (the newer), its program under its folder; OBS
    /// Studio with no install folder, found by its DisplayIcon; Notepad++ by App Paths; DaVinci
    /// Resolve installed with no folder recorded anywhere (installed, nothing to start); "FFmpeg
    /// 5.0.0 for Audacity" is not Audacity; "DaVinci Resolve Control Panels" is not DaVinci
    /// Resolve; an entry with no `detect` is never installed.
    ///
    /// Seen red 2026-10-19 with the candidates ranked by version alone (`rank` without
    /// `program.is_some()` first), and GIMP 3's program removed from its folder: GIMP 2.10.38 (its
    /// program there) lost to 3.2.4 (none), "the one whose program is there wins" got
    /// Some("3.2.4.0").
    #[test]
    fn the_matcher_against_windows_records() {
        let dir = crate::test_temp::dir("sw_matcher");
        let gimp2 = dir.join("GIMP 2");
        let gimp3 = dir.join("GIMP 3");
        let g2 = program(&gimp2, "bin/gimp-2.10.exe");
        let g3 = program(&gimp3, "bin/gimp-3.exe");
        let obs = program(&dir, "obs-studio/bin/64bit/obs64.exe");
        let npp = program(&dir, "Notepad++/notepad++.exe");
        let records = Records {
            uninstall: vec![
                record("GIMP 2.10.38", Some("2.10.38"), Some(&gimp2), Some(&g2.display().to_string())),
                record("GIMP 3.2.4", Some("3.2.4.0"), Some(&gimp3), Some(&g3.display().to_string())),
                record("OBS Studio", Some("32.2.2"), None, Some(&format!("{},0", obs.display()))),
                record("DaVinci Resolve", Some("20.0.00049"), None, None),
                record("DaVinci Resolve Control Panels", Some("2.3.2.0"), None, None),
                record("FFmpeg 5.0.0 for Audacity - x86_64", None, Some(&dir.join("ffmpeg")), None),
            ],
            app_paths: vec![AppPath { exe_name: "notepad++.exe".into(), path: npp.clone() }],
        };
        let gimp = find(&entry("GIMP", Some(r"^GIMP \d"), &["bin/gimp-3.exe", "bin/gimp-2.10.exe"], None), &records, &[]).expect("GIMP is installed");
        assert_eq!((gimp.version.as_deref(), gimp.program.as_ref()), (Some("3.2.4.0"), Some(&g3)), "the newer of the two");
        assert_eq!(gimp.launch, Some(Launch::Program { exe: g3.clone(), args: String::new(), dir: gimp3.join("bin") }));
        assert_eq!(gimp.folder.as_ref(), Some(&gimp3));

        let obs_found = find(&entry("OBS Studio", Some("^OBS Studio"), &["bin/64bit/obs64.exe"], None), &records, &[]).expect("OBS is installed");
        assert_eq!(obs_found.program.as_ref(), Some(&obs), "found by its DisplayIcon");
        assert_eq!(obs_found.folder.as_ref(), Some(&dir.join("obs-studio")), "its install folder, its exe path taken off");

        let npp_found = find(&entry("Notepad++", Some(r"^Notepad\+\+"), &["notepad++.exe"], None), &records, &[]).expect("Notepad++ by App Paths");
        assert_eq!((npp_found.via, npp_found.program.as_ref()), (Via::AppPaths, Some(&npp)));

        let resolve = find(&entry("DaVinci Resolve", Some("^DaVinci Resolve$"), &["Resolve.exe"], None), &records, &[]).expect("DaVinci Resolve is installed");
        assert_eq!((resolve.name_in_windows.as_deref(), resolve.version.as_deref()), (Some("DaVinci Resolve"), Some("20.0.00049")), "not its Control Panels");
        assert_eq!((resolve.program.as_ref(), resolve.launch.as_ref(), resolve.folder.as_ref()), (None, None, None), "installed, with nothing to start from here");

        assert_eq!(find(&entry("Audacity", Some(r"^Audacity( \d|$)"), &["Audacity.exe"], None), &records, &[]), None, "FFmpeg for Audacity is not Audacity");
        let none = ToolEntry { name: "Photopea".into(), ..ToolEntry::default() };
        assert_eq!(find(&none, &records, &[]), None, "no detect, never installed");

        // The red run's case: GIMP 3 listed, its program gone; GIMP 2's there.
        std::fs::remove_file(&g3).unwrap();
        let gimp = find(&entry("GIMP", Some(r"^GIMP \d"), &["bin/gimp-3.exe", "bin/gimp-2.10.exe"], None), &records, &[]).unwrap();
        assert_eq!(gimp.version.as_deref(), Some("2.10.38"), "the one whose program is there wins");
    }

    /// A COPY STEAM INSTALLED COUNTS: Blender from Steam (its app id fully installed) is installed
    /// through Steam, Launch sends Steam's own link, and its program is found in Steam's folder;
    /// one Steam is still downloading does not count; a copy in Windows' own list wins over Steam's.
    ///
    /// Seen red 2026-10-19 with Steam's state not checked (`g.state == State::Ready` taken out):
    /// "one Steam is still downloading does not count" got Some(Installed { via: Steam, .. }).
    #[test]
    fn a_copy_steam_installed_counts() {
        let dir = crate::test_temp::dir("sw_steam");
        let folder = dir.join("steamapps").join("common").join("Blender");
        let exe = program(&folder, "blender.exe");
        let game = |state: State| Game {
            store: crate::games::Store::Steam,
            id: "365670".into(),
            name: "Blender".into(),
            folder: folder.clone(),
            folder_here: true,
            state,
            play: (state == State::Ready).then(|| Launch::Link("steam://rungameid/365670".into())),
            show: folder.clone(),
            details: Default::default(),
        };
        let blender = entry("Blender", Some(r"^Blender( \d|$)"), &["blender.exe"], Some(365670));
        let found = find(&blender, &Records::default(), &[game(State::Ready)]).expect("Steam's copy counts");
        assert_eq!((found.via, found.launch.clone(), found.program.as_ref()), (Via::Steam, Some(Launch::Link("steam://rungameid/365670".into())), Some(&exe)));
        assert_eq!(find(&blender, &Records::default(), &[game(State::BeingInstalled)]), None, "one Steam is still downloading does not count");
        let own = dir.join("Blender Foundation");
        let own_exe = program(&own, "blender.exe");
        let records = Records { uninstall: vec![record("Blender", Some("5.2.2"), Some(&own), None)], app_paths: vec![] };
        let found = find(&blender, &records, &[game(State::Ready)]).unwrap();
        assert_eq!((found.via, found.program.as_ref()), (Via::Windows, Some(&own_exe)), "Windows' own copy wins");
    }

    /// A DISPLAYICON THAT IS AN UNINSTALLER IS NOT THE PROGRAM: with no `exe` in the data, a
    /// program DisplayIcon names is taken unless it looks like an uninstaller or a setup program.
    ///
    /// Seen red 2026-10-19 with `looks_like_installer` answering false: "an uninstaller is not the
    /// program" got Some(".../unins000.exe").
    #[test]
    fn an_uninstaller_is_not_the_program() {
        let dir = crate::test_temp::dir("sw_unins");
        let unins = program(&dir, "unins000.exe");
        let app = program(&dir, "wings3d.exe");
        let e = entry("Wings 3D", Some("^Wings 3D"), &[], None);
        let with = |icon: &Path| Records { uninstall: vec![record("Wings 3D 2.4.1", Some("2.4.1"), None, Some(&icon.display().to_string()))], app_paths: vec![] };
        assert_eq!(find(&e, &with(&unins), &[]).unwrap().program, None, "an uninstaller is not the program");
        assert_eq!(find(&e, &with(&app), &[]).unwrap().program, Some(app));
    }
}
