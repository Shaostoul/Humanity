//! THE ONE way the Apps page (its games and its other programs) starts anything: a store's link, a
//! game's or a program's own program, a program's Start-menu shortcut, or the computer's file
//! manager showing a folder. Everything on the Apps page that leaves the app goes through
//! [`launch`], and only when the person clicks (web pages open in the browser through egui).
//!
//! TEST BUILDS NEVER START ANYTHING. In a test build `launch` records what it was asked and returns
//! (`recorded` reads the record back), and the code that talks to the computer is not even
//! compiled in, so no test, snapshot or page drawn in a test can start a game, a store's client or
//! a `steam://` link, whatever it clicks.

use std::path::{Path, PathBuf};

/// Something the Apps page asks the computer to start or show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launch {
    /// A store's own link: `steam://...` or `com.epicgames.launcher://...`, handed to whatever the
    /// computer has registered for it (the store's own app).
    Link(String),
    /// A game's own program, its arguments passed as written, started in `dir`.
    Program { exe: PathBuf, args: String, dir: PathBuf },
    /// Show this file or folder in the computer's file manager (selected, on Windows).
    ShowInFolder(PathBuf),
    /// Open a program's Start-menu shortcut (a `.lnk` file), as a click on it in the Start menu
    /// does: for a program whose shortcut names no program file to start directly (2026-10-19,
    /// software/shortcuts.rs).
    Shortcut(PathBuf),
}

/// The links the page may send: only the two stores' own, so nothing else can ever be opened here.
pub const LINK_SCHEMES: [&str; 2] = ["steam://", "com.epicgames.launcher://"];

/// Start or show `what`. `Err` carries the plain words for why it did not start.
pub fn launch(what: &Launch) -> Result<(), String> {
    #[cfg(test)]
    {
        recorder::record(what)
    }
    #[cfg(not(test))]
    {
        real(what)
    }
}

/// What the computer does with `what` (not compiled into test builds).
#[cfg(not(test))]
fn real(what: &Launch) -> Result<(), String> {
    match what {
        Launch::Link(url) => open_link(url),
        Launch::Program { exe, args, dir } => start_program(exe, args, dir),
        Launch::ShowInFolder(path) => {
            crate::gui::widgets::file_caution::show_in_folder(path);
            Ok(())
        }
        Launch::Shortcut(lnk) => open_shortcut(lnk),
    }
}

/// Open a Start-menu shortcut with Explorer, as a double-click on it does. Only a `.lnk` file that
/// is there; no shell runs, so nothing in its name is read as a command.
#[cfg(not(test))]
fn open_shortcut(lnk: &Path) -> Result<(), String> {
    if !lnk.extension().is_some_and(|x| x.eq_ignore_ascii_case("lnk")) || !lnk.is_file() {
        return Err("its Start-menu shortcut is not there".to_string());
    }
    #[cfg(windows)]
    {
        std::process::Command::new("explorer").arg(lnk).spawn().map(|_| ()).map_err(|e| format!("Windows could not open its shortcut ({e})"))
    }
    #[cfg(not(windows))]
    {
        Err("a Windows shortcut opens only on Windows".to_string())
    }
}

/// Hand a store's link to the app the computer has registered for it.
#[cfg(not(test))]
fn open_link(url: &str) -> Result<(), String> {
    if !LINK_SCHEMES.iter().any(|s| url.starts_with(s)) {
        return Err("that is not a link to a game store's own app".to_string());
    }
    // Explorer opens a link with the program registered for its scheme, as a click on a desktop
    // shortcut to it does. No shell runs, so nothing in the link is read as a command.
    #[cfg(target_os = "windows")]
    let mut cmd = std::process::Command::new("explorer");
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let mut cmd = std::process::Command::new("xdg-open");
    cmd.arg(url).spawn().map(|_| ()).map_err(|e| format!("the store's app could not be opened ({e})"))
}

/// Start a game's own program in `dir` (its own folder when `dir` is not there).
#[cfg(not(test))]
fn start_program(exe: &Path, args: &str, dir: &Path) -> Result<(), String> {
    use std::process::{Command, Stdio};
    if !exe.is_file() {
        return Err("its program is not there".to_string());
    }
    let dir = if dir.is_dir() { dir.to_path_buf() } else { exe.parent().map(Path::to_path_buf).unwrap_or_default() };
    let mut cmd = Command::new(exe);
    cmd.current_dir(&dir).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        // GOG writes a game's arguments as one Windows command-line string; it is passed exactly
        // as written rather than split and re-quoted.
        use std::os::windows::process::CommandExt;
        if !args.trim().is_empty() {
            cmd.raw_arg(args.trim());
        }
    }
    #[cfg(not(windows))]
    cmd.args(args.split_whitespace());
    match cmd.spawn() {
        Ok(_) => Ok(()),
        // ERROR_ELEVATION_REQUIRED: the game asks to run as administrator, which only its own
        // shortcut (or Explorer) can ask Windows for.
        Err(e) if e.raw_os_error() == Some(740) => Err("Windows says it must run as an administrator: start it from its own shortcut".to_string()),
        Err(e) => Err(format!("Windows could not start it ({e})")),
    }
}

/// Test builds: what `launch` was asked, per test thread, instead of starting anything.
#[cfg(test)]
pub(crate) mod recorder {
    use super::Launch;
    use std::cell::RefCell;

    thread_local! {
        static ASKED: RefCell<Vec<Launch>> = const { RefCell::new(Vec::new()) };
        static ANSWER: RefCell<Result<(), String>> = const { RefCell::new(Ok(())) };
    }

    pub(super) fn record(what: &Launch) -> Result<(), String> {
        ASKED.with(|a| a.borrow_mut().push(what.clone()));
        ANSWER.with(|r| r.borrow().clone())
    }

    /// Everything `launch` was asked on this thread since the last call, in order.
    pub(crate) fn recorded() -> Vec<Launch> {
        ASKED.with(|a| std::mem::take(&mut *a.borrow_mut()))
    }

    /// From now on, on this thread, `launch` answers `answer` (a game that would not start).
    pub(crate) fn answer_with(answer: Result<(), String>) {
        ANSWER.with(|r| *r.borrow_mut() = answer);
    }
}
