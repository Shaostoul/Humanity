//! Central hot-reload coordinator — watches files and notifies subsystems.

pub mod data_store;

/// Coordinates hot-reload across shaders, assets, and data files.
pub struct HotReloadCoordinator {
    // TODO: channel to subsystems, file watcher
}

impl HotReloadCoordinator {
    pub fn new() -> Self {
        Self {}
    }
}
