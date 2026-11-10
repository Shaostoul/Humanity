//! Where the AI engine is (docs/design/embeddable-ai.md, 7.1): once chosen, in the person's own files
//! (`%APPDATA%\HumanityOS\ai\engine\<version>\humanity-ai.exe` when HumanityOS keeps them there, beside
//! the app in portable mode). Removing AI is deleting the `ai` folder (5.3).
//!
//! For development there are two more places, never counted as "installed" by Settings > AI: the
//! engine `just ai-engine` builds in this checkout (target/ai-engine/), and `HUMANITY_AI_ENGINE`, a full
//! path to an engine. The app only ever starts an engine by its full path (Microsoft's guidance the
//! runtime finding quotes in F10: "specify a fully qualified path when using ... CreateProcess").

use std::path::{Path, PathBuf};

use humanity_core::storage::{self, StorageMode};

/// The engine version this app expects (the catalog names it from rung 2, 7.4). The engine reports its
/// own in its `hello`; a different one is shown on the Dev page, while a different PROTOCOL is refused.
pub const ENGINE_VERSION: &str = "1.0.0";

/// The engine program's file name.
pub const ENGINE_EXE: &str = if cfg!(windows) { "humanity-ai.exe" } else { "humanity-ai" };

/// The `ai` folder in the person's own files, or None while they have not chosen where HumanityOS
/// keeps their files (nothing may be written before that choice, storage.rs).
pub fn ai_root() -> Option<PathBuf> {
    match storage::mode() {
        StorageMode::Portable => Some(storage::exe_dir().join("ai")),
        StorageMode::Installed | StorageMode::LegacyBesideExe => storage::os_root().map(|r| r.join("ai")),
        StorageMode::Undecided => None,
    }
}

/// Where the engine of [`ENGINE_VERSION`] sits under an `ai` folder.
pub fn engine_path_in(ai_root: &Path) -> PathBuf {
    ai_root.join("engine").join(ENGINE_VERSION).join(ENGINE_EXE)
}

/// The installed engine, if the person has one (never the developer's).
pub fn installed_engine() -> Option<PathBuf> {
    ai_root().map(|r| engine_path_in(&r)).filter(|p| p.is_file())
}

/// Where a developer's engine came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DevSource {
    /// `HUMANITY_AI_ENGINE`.
    Variable,
    /// Built in this checkout by `just ai-engine` (release) or by the client tests (debug).
    Built,
}

/// A full path, or why it is not one (a relative path would be looked up from wherever the app happens
/// to be started, the very search Microsoft's guidance warns against).
pub fn full_path(p: &Path) -> Result<PathBuf, String> {
    if !p.is_absolute() {
        return Err(format!("{} is not a full path", p.display()));
    }
    if !p.is_file() {
        return Err(format!("{} is not a file", p.display()));
    }
    Ok(p.to_path_buf())
}

/// The developer's engine: `HUMANITY_AI_ENGINE` if it names one (a relative path is refused), else the
/// release then the debug build in this checkout's target/ai-engine/. None when there is neither.
pub fn dev_engine() -> Option<(PathBuf, DevSource)> {
    if let Some(v) = std::env::var_os("HUMANITY_AI_ENGINE") {
        match full_path(Path::new(&v)) {
            Ok(p) => return Some((p, DevSource::Variable)),
            Err(e) => log::warn!("[AI] HUMANITY_AI_ENGINE ignored: {e}"),
        }
    }
    let target = Path::new(env!("HUMANITY_REPO_ROOT")).join("target").join("ai-engine");
    ["release", "debug"].iter().map(|profile| target.join(profile).join(ENGINE_EXE)).find(|p| p.is_file()).map(|p| (p, DevSource::Built))
}
