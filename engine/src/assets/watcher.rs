//! File watcher — monitors the data directory for changes via notify.
//!
//! When a file changes, the AssetManager's cache is invalidated for that path.
//! The game re-loads the data on the next access (lazy reload).

use std::path::PathBuf;
use std::sync::mpsc;

/// Watches the data directory and reports changed file paths.
pub struct FileWatcher {
    _watcher: notify::RecommendedWatcher,
    change_rx: mpsc::Receiver<PathBuf>,
    data_dir: PathBuf,
}

impl FileWatcher {
    /// Start watching the given directory recursively.
    pub fn new(data_dir: PathBuf) -> Result<Self, String> {
        use notify::{RecursiveMode, Watcher};

        let (tx, rx) = mpsc::channel();

        let sender = tx;
        let mut watcher = notify::recommended_watcher(
            move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    // Only care about modifications and creates
                    use notify::EventKind;
                    match event.kind {
                        EventKind::Modify(_) | EventKind::Create(_) => {
                            for path in event.paths {
                                let _ = sender.send(path);
                            }
                        }
                        _ => {}
                    }
                }
            },
        )
        .map_err(|e| format!("Failed to create file watcher: {e}"))?;

        watcher
            .watch(&data_dir, RecursiveMode::Recursive)
            .map_err(|e| format!("Failed to watch {}: {e}", data_dir.display()))?;

        log::info!("FileWatcher: watching {}", data_dir.display());

        Ok(Self {
            _watcher: watcher,
            change_rx: rx,
            data_dir,
        })
    }

    /// Poll for changed files. Returns relative paths from the data directory.
    /// Call once per frame (or periodically) to check for changes.
    pub fn poll_changes(&self) -> Vec<String> {
        let mut changed = Vec::new();
        while let Ok(path) = self.change_rx.try_recv() {
            // Convert to relative path from data directory
            if let Ok(relative) = path.strip_prefix(&self.data_dir) {
                if let Some(rel_str) = relative.to_str() {
                    // Normalize path separators
                    let normalized = rel_str.replace('\\', "/");
                    if !changed.contains(&normalized) {
                        changed.push(normalized);
                    }
                }
            }
        }
        changed
    }
}
