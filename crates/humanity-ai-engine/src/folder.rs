//! The engine's own folder, and the only place it loads a library from (7.2; the runtime finding,
//! F10, quoting Microsoft's "Dynamic-Link Library Security": "Wherever possible, specify a fully
//! qualified path when using the LoadLibrary, LoadLibraryEx, CreateProcess, or ShellExecute
//! functions.").
//!
//! Rung 1 loads no library at all. From rung 2 every runtime library (audio.cpp's C library, ONNX
//! Runtime if tract ever needs its fallback) is loaded through [`library_path`], by full path from the
//! engine's own folder, and at start [`restrict_library_search`] takes the current folder and PATH out
//! of Windows' search for any library loaded later, so a planted copy elsewhere is never picked up.

use std::path::{Path, PathBuf};

/// The folder the running engine lives in (`%APPDATA%\HumanityOS\ai\engine\<version>\` once
/// installed), or None when the operating system cannot say.
pub fn own_folder() -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf))
}

/// The full path of a library beside the engine. Refuses a name that is a path (a separator, `..`, a
/// drive), so nothing outside the engine's folder can be named through it.
pub fn library_path(folder: &Path, name: &str) -> Result<PathBuf, String> {
    let bad = name.is_empty() || name.contains(['/', '\\', ':']) || name == "." || name == ".." || name.starts_with('.');
    if bad {
        return Err(format!("{name:?} is not a plain library file name"));
    }
    if !folder.is_absolute() {
        return Err(format!("{} is not a full path", folder.display()));
    }
    Ok(folder.join(name))
}

/// From now on, Windows looks for a library this process loads only in the engine's own folder and
/// in System32: never the current folder, never PATH (`SetDefaultDllDirectories`). Does nothing on
/// other systems, whose loaders do not search the current folder. Returns whether it took effect.
pub fn restrict_library_search() -> bool {
    #[cfg(windows)]
    {
        // LOAD_LIBRARY_SEARCH_APPLICATION_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32 (libloaderapi.h).
        const APPLICATION_DIR: u32 = 0x0000_0200;
        const SYSTEM32: u32 = 0x0000_0800;
        #[link(name = "kernel32")]
        extern "system" {
            fn SetDefaultDllDirectories(flags: u32) -> i32;
        }
        // SAFETY: a documented kernel32 function taking flags by value; it changes only this
        // process's library search order.
        unsafe { SetDefaultDllDirectories(APPLICATION_DIR | SYSTEM32) != 0 }
    }
    #[cfg(not(windows))]
    {
        false
    }
}
