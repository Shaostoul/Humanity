//! The Start menu's shortcuts (2026-10-19; the operator: "why can't we launch DaVinci Resolve from
//! our launcher? Can we not auto-locate the EXE to link to?").
//!
//! Some installers put a program in Windows' installed-programs list without saying where its
//! program file is: DaVinci Resolve's record has no install folder and no DisplayIcon naming its
//! program, and it is not in App Paths. But every one of them puts a shortcut in the Start menu,
//! and that shortcut names the program (Resolve's names `C:\Apps\DaVinci\Resolve.exe` on the
//! operator's computer). So when a record gives no program file (software/matcher.rs), the Start
//! menu's shortcut with the entry's name is read and its program is what Launch starts, through
//! the one launcher (games::launch). A shortcut whose program cannot be read is started itself.
//!
//! READ ONLY, on the Apps page's worker thread:
//! - The two Start-menu folders are listed, the one for everyone
//!   (`%ProgramData%\Microsoft\Windows\Start Menu\Programs`) and this person's
//!   (`%APPDATA%\Microsoft\Windows\Start Menu\Programs`), folders inside them too, by name only.
//! - Only a shortcut whose name matches an entry is opened, with Windows' own shortcut reader
//!   (IShellLinkW, loaded with STGM_READ): its program, its arguments and its working folder are
//!   read. It is never saved, and never "resolved" (that can search the disk for a moved program
//!   and open a window), so nothing about it changes.
//!
//! Elsewhere than Windows nothing is listed or read.

use std::path::{Path, PathBuf};

/// A shortcut in the Start menu, by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shortcut {
    /// Its name as the Start menu shows it: the file name without ".lnk" ("DaVinci Resolve").
    pub name: String,
    /// The shortcut file itself.
    pub path: PathBuf,
}

/// What a shortcut starts, as it is written in it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Target {
    /// Its program file, when it names one (an installer's "advertised" shortcut may not).
    pub program: Option<PathBuf>,
    /// The arguments it passes, as written.
    pub args: String,
    /// The folder it starts the program in, when it names one.
    pub dir: Option<PathBuf>,
}

/// At most this many shortcuts are listed, and folders this deep looked into.
const MOST: usize = 5_000;
const DEEPEST: usize = 4;

/// The two Start-menu folders on this computer: everyone's, then this person's (each when its
/// environment variable is set).
pub fn start_menu_folders() -> Vec<PathBuf> {
    let under = |var: &str| std::env::var_os(var).map(|v| PathBuf::from(v).join("Microsoft").join("Windows").join("Start Menu").join("Programs"));
    ["ProgramData", "APPDATA"].iter().filter_map(|v| under(v)).collect()
}

/// Every shortcut in `folders` and the folders inside them, by name; read only.
pub fn list(folders: &[PathBuf]) -> Vec<Shortcut> {
    let mut out = Vec::new();
    let mut todo: Vec<(PathBuf, usize)> = folders.iter().map(|f| (f.clone(), 0)).collect();
    while let Some((dir, depth)) = todo.pop() {
        let mut entries = crate::games::list_dir(&dir);
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            if out.len() >= MOST {
                return out;
            }
            let Ok(kind) = entry.file_type() else { continue };
            let path = entry.path();
            if kind.is_dir() && depth < DEEPEST {
                todo.push((path, depth + 1));
            } else if kind.is_file() && path.extension().is_some_and(|x| x.eq_ignore_ascii_case("lnk")) {
                let name = path.file_stem().map(|s| s.to_string_lossy().trim().to_string()).unwrap_or_default();
                if !name.is_empty() {
                    out.push(Shortcut { name, path });
                }
            }
        }
    }
    out
}

/// What the shortcut `lnk` starts, read with Windows' own shortcut reader. `None` when it does not
/// read (not a shortcut, or elsewhere than Windows).
pub fn target(lnk: &Path) -> Option<Target> {
    crate::games::note_disk_read();
    #[cfg(windows)]
    {
        windows_link::read(lnk)
    }
    #[cfg(not(windows))]
    {
        let _ = lnk;
        None
    }
}

/// Reading (and, for the tests, writing) a shortcut through IShellLinkW. The thread joins COM's
/// multithreaded apartment for the call and leaves it after, as media/system.rs does: a thread
/// already in another apartment carries on in it without leaving.
#[cfg(windows)]
mod windows_link {
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};

    use windows::core::{Interface, PCWSTR};
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, STGM_READ};
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    use super::Target;

    /// The longest path or argument line read (the shortcut format's own limit is far below it).
    const ROOM: usize = 4096;

    fn wide(p: &Path) -> Vec<u16> {
        p.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
    }

    /// The text before the first NUL.
    fn text(buf: &[u16]) -> String {
        let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..end]).trim().to_string()
    }

    /// Run `f` with this thread in COM, leaving after only if this call joined.
    fn in_com<T>(f: impl FnOnce() -> Option<T>) -> Option<T> {
        // SAFETY: CoInitializeEx with no reserved pointer; S_OK or S_FALSE is balanced below,
        // RPC_E_CHANGED_MODE (already in another apartment) is not.
        let joined = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
        let out = f();
        if joined {
            // SAFETY: balances this call's own successful CoInitializeEx, on the same thread,
            // after every COM object `f` made has been dropped.
            unsafe { CoUninitialize() };
        }
        out
    }

    pub(super) fn read(lnk: &Path) -> Option<Target> {
        if !lnk.is_file() {
            return None;
        }
        let name = wide(lnk);
        in_com(|| {
            // SAFETY: the shell's own shortcut object, in process; `name` is NUL-ended and
            // outlives the calls; each buffer is passed with its own length.
            unsafe {
                let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
                let file: IPersistFile = link.cast().ok()?;
                file.Load(PCWSTR(name.as_ptr()), STGM_READ).ok()?;
                let mut path = vec![0u16; ROOM];
                // Flags 0: the program's ordinary long path. An advertised shortcut has none.
                let program = link.GetPath(&mut path, std::ptr::null_mut(), 0).ok().map(|_| text(&path)).filter(|p| !p.is_empty()).map(PathBuf::from);
                let mut args = vec![0u16; ROOM];
                let args = link.GetArguments(&mut args).ok().map(|_| text(&args)).unwrap_or_default();
                let mut dir = vec![0u16; ROOM];
                let dir = link.GetWorkingDirectory(&mut dir).ok().map(|_| text(&dir)).filter(|d| !d.is_empty()).map(PathBuf::from);
                Some(Target { program, args, dir })
            }
        })
    }

    /// Test builds: write a real shortcut at `lnk` to `program`, with Windows' own writer (the
    /// fixtures the reader is tested on).
    #[cfg(test)]
    pub(crate) fn write(lnk: &Path, program: &Path, args: &str, dir: &Path) -> bool {
        let (name, program, dir) = (wide(lnk), wide(program), wide(dir));
        let args: Vec<u16> = args.encode_utf16().chain(std::iter::once(0)).collect();
        in_com(|| {
            // SAFETY: as in `read`; every string is NUL-ended and outlives its call.
            unsafe {
                let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
                link.SetPath(PCWSTR(program.as_ptr())).ok()?;
                link.SetArguments(PCWSTR(args.as_ptr())).ok()?;
                link.SetWorkingDirectory(PCWSTR(dir.as_ptr())).ok()?;
                let file: IPersistFile = link.cast().ok()?;
                file.Save(PCWSTR(name.as_ptr()), true).ok()?;
                Some(())
            }
        })
        .is_some()
    }
}

/// Test builds: write a real Start-menu shortcut (Windows' own writer), for the fixtures.
#[cfg(all(test, windows))]
pub(crate) fn write_for_test(lnk: &Path, program: &Path, args: &str, dir: &Path) {
    std::fs::create_dir_all(lnk.parent().unwrap()).unwrap();
    assert!(windows_link::write(lnk, program, args, dir), "the shortcut {} was written", lnk.display());
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// A START-MENU SHORTCUT READS BACK AS WRITTEN: a real shortcut made with Windows' own writer
    /// (IShellLinkW, IPersistFile::Save) in a scratch Start menu, in a folder inside it as
    /// DaVinci Resolve's is, is listed by its name, and its program, arguments and working folder
    /// are read; a file named .lnk that is not a shortcut reads as nothing; a folder that is not
    /// there lists nothing.
    ///
    /// Seen red 2026-10-19 with the folders inside the Start menu not looked into (`depth <
    /// DEEPEST` as `depth < 0`): "listed by its name" found [] (the shortcut is one folder down).
    #[test]
    fn a_start_menu_shortcut_reads_back_as_written() {
        let dir = crate::test_temp::dir("sw_shortcut");
        let program = dir.join("Apps").join("DaVinci").join("Resolve.exe");
        std::fs::create_dir_all(program.parent().unwrap()).unwrap();
        std::fs::write(&program, "x").unwrap();
        let menu = dir.join("Start Menu").join("Programs");
        let lnk = menu.join("Blackmagic Design").join("DaVinci Resolve").join("DaVinci Resolve.lnk");
        write_for_test(&lnk, &program, "-nosplash", program.parent().unwrap());
        std::fs::write(menu.join("Not a shortcut.lnk"), "x").unwrap();

        let found = list(&[menu.clone(), dir.join("no such folder")]);
        let names: Vec<&str> = found.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"DaVinci Resolve"), "listed by its name: {names:?}");
        let resolve = found.iter().find(|s| s.name == "DaVinci Resolve").unwrap();
        assert_eq!(resolve.path, lnk);
        let t = target(&resolve.path).expect("it reads");
        assert_eq!(t.program.as_deref(), Some(program.as_path()), "its program");
        assert_eq!(t.args, "-nosplash", "its arguments");
        assert_eq!(t.dir.as_deref(), program.parent(), "its working folder");
        assert_eq!(target(&menu.join("Not a shortcut.lnk")), None, "a file that is not a shortcut reads as nothing");
        assert!(lnk.is_file(), "reading it leaves it where it was");
    }
}
