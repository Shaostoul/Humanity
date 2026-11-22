//! File system operations for the data/ directory.
//!
//! Provides safe read/write/list access restricted to the `data/` directory.
//! All paths are validated to prevent directory traversal attacks.

use std::path::{Path, PathBuf};

/// Allowed text file extensions for read/write operations.
const ALLOWED_TEXT_EXTENSIONS: &[&str] = &[
    "txt", "md", "rs", "js", "py", "toml", "json", "csv", "html", "css",
    "ron", "yaml", "yml", "xml", "cfg", "ini", "sh", "bat",
];

/// Maximum file size for read/write operations (1MB).
const MAX_FILE_SIZE: u64 = 1_048_576;

/// Validate that a path is safe (no traversal) and within the data/ directory.
/// Returns the canonicalized absolute path if valid.
pub fn validate_path(requested_path: &str) -> Result<PathBuf, String> {
    // Reject empty paths.
    if requested_path.is_empty() {
        return Err("Path cannot be empty".into());
    }

    // Reject path traversal attempts.
    if requested_path.contains("..") {
        return Err("Path traversal not allowed".into());
    }

    // Reject absolute paths and paths starting with / or \
    if requested_path.starts_with('/') || requested_path.starts_with('\\') {
        return Err("Absolute paths not allowed".into());
    }

    // The path must start with "data/" or be exactly "data".
    if !requested_path.starts_with("data/") && requested_path != "data" {
        return Err("Access restricted to data/ directory".into());
    }

    // Build the path relative to the working directory.
    let path = PathBuf::from(requested_path);

    // Check that it doesn't escape via symlinks (if the target exists).
    if path.exists() {
        let canonical = path.canonicalize().map_err(|e| format!("Cannot resolve path: {e}"))?;
        let data_dir = PathBuf::from("data").canonicalize().map_err(|e| format!("Cannot resolve data dir: {e}"))?;
        if !canonical.starts_with(&data_dir) {
            return Err("Path resolves outside data/ directory".into());
        }
        if in_server_only_dir(&canonical, &data_dir) {
            return Err(SERVER_ONLY_WORDS.into());
        }
        Ok(canonical)
    } else {
        // For new files, ensure the parent exists and is inside data/.
        if let Some(parent) = path.parent() {
            if parent.exists() {
                let canonical_parent = parent.canonicalize().map_err(|e| format!("Cannot resolve parent: {e}"))?;
                let data_dir = PathBuf::from("data").canonicalize().map_err(|e| format!("Cannot resolve data dir: {e}"))?;
                if !canonical_parent.starts_with(&data_dir) {
                    return Err("Path resolves outside data/ directory".into());
                }
                // Return the parent canonical + filename.
                let filename = path.file_name().ok_or("Invalid filename")?;
                let joined = canonical_parent.join(filename);
                if in_server_only_dir(&joined, &data_dir) {
                    return Err(SERVER_ONLY_WORDS.into());
                }
                Ok(joined)
            } else {
                Err("Parent directory does not exist".into())
            }
        } else {
            Err("Invalid path".into())
        }
    }
}

/// The files these routes never list, read or write, for anyone, admins included: the owner
/// claim code (handlers/owner.rs), whose reader could make themselves the server's owner. It is for
/// the server machine's eyes alone (BUG-346: an admin could read it here and claim the owner role).
pub const SERVER_ONLY: &[&str] = &["owner-claim-code.txt"];

/// The folders under data/ these routes never list, read or write into, for anyone: the held
/// copies (storage/held.rs), whose folder names are the package ids each part is bound to and
/// whose sealed parts are for the owner's device alone (the owner review's finding 1, 2026-11-20:
/// the data-file route listed the package folders).
pub const SERVER_ONLY_DIRS: &[&str] = &["held"];

/// Whether `path` (resolved) lies in one of the [`SERVER_ONLY_DIRS`] under `data_dir`, or is one.
pub fn in_server_only_dir(path: &Path, data_dir: &Path) -> bool {
    let Ok(rest) = path.strip_prefix(data_dir) else { return false };
    rest.components().next().is_some_and(|c| SERVER_ONLY_DIRS.iter().any(|d| c.as_os_str().to_string_lossy().eq_ignore_ascii_case(d)))
}

/// Whether `path` names a [`SERVER_ONLY`] file.
pub fn is_server_only(path: &Path) -> bool {
    path.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).is_some_and(|n| SERVER_ONLY.contains(&n.as_str()))
}

const SERVER_ONLY_WORDS: &str = "This file is for the server machine's eyes alone: it is never opened or changed through the app.";

/// Check if a file extension is in the allowed text extensions list.
pub fn is_text_extension(ext: &str) -> bool {
    ALLOWED_TEXT_EXTENSIONS.contains(&ext.to_lowercase().as_str())
}

/// File entry returned by list operations.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub modified: u64,
    pub is_directory: bool,
    pub extension: String,
}

/// List files in a directory within data/.
pub fn list_directory(dir_path: &str) -> Result<Vec<FileEntry>, String> {
    let abs_path = validate_path(dir_path)?;

    if !abs_path.is_dir() {
        return Err("Path is not a directory".into());
    }

    let mut entries = Vec::new();
    let read_dir = std::fs::read_dir(&abs_path)
        .map_err(|e| format!("Cannot read directory: {e}"))?;

    // We need the data dir prefix to compute relative paths.
    let data_dir = PathBuf::from("data").canonicalize()
        .map_err(|e| format!("Cannot resolve data dir: {e}"))?;

    for entry in read_dir {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files, and the files and folders for the server machine alone.
        if name.starts_with('.') || is_server_only(Path::new(&name)) || SERVER_ONLY_DIRS.iter().any(|d| name.eq_ignore_ascii_case(d)) {
            continue;
        }

        let is_directory = metadata.is_dir();
        let size = if is_directory { 0 } else { metadata.len() };
        let modified = metadata.modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let extension = if is_directory {
            String::new()
        } else {
            Path::new(&name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase()
        };

        // Compute relative path from data/ root.
        let full_path = entry.path().canonicalize().unwrap_or_else(|_| entry.path());
        let relative = full_path.strip_prefix(&data_dir)
            .map(|p| format!("data/{}", p.to_string_lossy().replace('\\', "/")))
            .unwrap_or_else(|_| format!("data/{}", name));

        entries.push(FileEntry {
            name,
            path: relative,
            size,
            modified,
            is_directory,
            extension,
        });
    }

    // Sort: directories first, then alphabetical.
    entries.sort_by(|a, b| {
        b.is_directory.cmp(&a.is_directory)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(entries)
}

/// Read a text file's contents.
pub fn read_file(file_path: &str) -> Result<String, String> {
    let abs_path = validate_path(file_path)?;

    if abs_path.is_dir() {
        return Err("Cannot read a directory".into());
    }
    if is_server_only(&abs_path) {
        return Err(SERVER_ONLY_WORDS.into());
    }

    // Check extension.
    let ext = abs_path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if !is_text_extension(ext) {
        return Err(format!("File type .{ext} is not allowed for reading"));
    }

    // Check size.
    let metadata = std::fs::metadata(&abs_path)
        .map_err(|e| format!("Cannot read file metadata: {e}"))?;
    if metadata.len() > MAX_FILE_SIZE {
        return Err(format!("File too large ({} bytes, max {})", metadata.len(), MAX_FILE_SIZE));
    }

    std::fs::read_to_string(&abs_path)
        .map_err(|e| format!("Cannot read file: {e}"))
}

/// Write content to a text file.
pub fn write_file(file_path: &str, content: &str) -> Result<(), String> {
    let abs_path = validate_path(file_path)?;

    if abs_path.is_dir() {
        return Err("Cannot write to a directory".into());
    }
    if is_server_only(&abs_path) {
        return Err(SERVER_ONLY_WORDS.into());
    }

    // Check extension.
    let ext = abs_path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if !is_text_extension(ext) {
        return Err(format!("File type .{ext} is not allowed for writing"));
    }

    // Check content size.
    if content.len() as u64 > MAX_FILE_SIZE {
        return Err(format!("Content too large ({} bytes, max {})", content.len(), MAX_FILE_SIZE));
    }

    std::fs::write(&abs_path, content)
        .map_err(|e| format!("Cannot write file: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE HELD FOLDER AND THE CLAIM CODE ARE NEVER REACHED THROUGH THESE ROUTES (the owner review's
    /// finding 1, 2026-11-20; BUG-346): data/held itself and everything in it is refused, in any
    /// letter case, while a name that merely starts with the same letters is not; the claim code
    /// file is refused by name.
    ///
    /// Seen red 2026-11-20 with SERVER_ONLY_DIRS emptied (the branch as reviewed): "the held folder"
    /// was allowed.
    #[test]
    fn the_held_folder_and_the_claim_code_are_for_the_server_alone() {
        let data = Path::new("/srv/hos/data");
        assert!(in_server_only_dir(&data.join("held"), data), "the held folder");
        assert!(in_server_only_dir(&data.join("held").join("0123abcd").join("0.sealed"), data), "a sealed part");
        assert!(in_server_only_dir(&data.join("Held").join(".handover-0123abcd"), data), "any letter case");
        assert!(!in_server_only_dir(&data.join("heldover.txt"), data), "a name that only starts the same");
        assert!(!in_server_only_dir(&data.join("plants.csv"), data));
        assert!(is_server_only(Path::new("data/owner-claim-code.txt")) && is_server_only(Path::new("data/OWNER-CLAIM-CODE.TXT")));
        assert!(!is_server_only(Path::new("data/plants.csv")));
    }
}
