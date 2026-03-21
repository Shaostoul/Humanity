//! File watcher — monitors asset directories for changes via notify (native only).

/// Watches asset directories and emits reload events.
#[cfg(feature = "native")]
pub struct FileWatcher {
    // TODO: notify::RecommendedWatcher, event channel
}

#[cfg(feature = "native")]
impl FileWatcher {
    pub fn new() -> Self {
        Self {}
    }
}
