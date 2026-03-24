//! Mod support framework for HumanityOS
//!
//! Mods are directories in `data/mods/{mod-name}/` that can override
//! any base data file by mirroring its path structure.
//! Each mod contains a `mod.json` manifest describing its metadata
//! and load order.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Manifest loaded from a mod's `mod.json` file.
#[derive(Debug, Clone, Deserialize)]
pub struct ModManifest {
    /// Human-readable mod name
    pub name: String,
    /// Unique identifier (matches directory name)
    pub id: String,
    /// Semver version string
    pub version: String,
    /// Mod author
    pub author: String,
    /// Short description of the mod
    pub description: String,
    /// Load order priority (lower loads first, default 100)
    #[serde(default = "default_load_order")]
    pub load_order: i32,
    /// List of mod IDs this mod depends on
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Filesystem path to the mod directory (populated at scan time, not from JSON)
    #[serde(skip)]
    pub path: PathBuf,
}

fn default_load_order() -> i32 {
    100
}

/// Scans, sorts, and resolves mod-overridden file paths.
pub struct ModLoader;

impl ModLoader {
    /// Scan the mods directory for valid mod directories containing `mod.json`.
    /// Returns a list of parsed manifests with their filesystem paths set.
    pub fn scan_mods(mods_dir: &Path) -> Vec<ModManifest> {
        let mut mods = Vec::new();

        let entries = match std::fs::read_dir(mods_dir) {
            Ok(e) => e,
            Err(e) => {
                log::warn!("Could not read mods directory {}: {}", mods_dir.display(), e);
                return mods;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let manifest_path = path.join("mod.json");
            if !manifest_path.exists() {
                continue;
            }

            match std::fs::read_to_string(&manifest_path) {
                Ok(content) => match serde_json::from_str::<ModManifest>(&content) {
                    Ok(mut manifest) => {
                        manifest.path = path;
                        log::info!(
                            "Found mod: {} v{} by {} (order: {})",
                            manifest.name,
                            manifest.version,
                            manifest.author,
                            manifest.load_order
                        );
                        mods.push(manifest);
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to parse {}: {}",
                            manifest_path.display(),
                            e
                        );
                    }
                },
                Err(e) => {
                    log::warn!(
                        "Failed to read {}: {}",
                        manifest_path.display(),
                        e
                    );
                }
            }
        }

        mods
    }

    /// Sort mods by load_order (ascending), then alphabetically by id for ties.
    /// Also performs basic dependency validation (warns on missing deps).
    pub fn load_order(mods: &[ModManifest]) -> Vec<String> {
        let mut sorted: Vec<&ModManifest> = mods.iter().collect();
        sorted.sort_by(|a, b| {
            a.load_order
                .cmp(&b.load_order)
                .then_with(|| a.id.cmp(&b.id))
        });

        // Warn about missing dependencies
        let ids: std::collections::HashSet<&str> =
            mods.iter().map(|m| m.id.as_str()).collect();
        for m in &sorted {
            for dep in &m.dependencies {
                if !ids.contains(dep.as_str()) {
                    log::warn!(
                        "Mod '{}' depends on '{}' which is not installed",
                        m.id, dep
                    );
                }
            }
        }

        sorted.iter().map(|m| m.id.clone()).collect()
    }

    /// Resolve a relative data path through the mod chain.
    /// Checks mods in reverse load order (last mod wins) for the file,
    /// falling back to the base data directory.
    ///
    /// # Arguments
    /// * `base` - The base data directory (e.g. `data/`)
    /// * `mods` - Sorted list of mod manifests (in load order)
    /// * `relative_path` - Path relative to data dir (e.g. `items.csv`)
    ///
    /// # Returns
    /// The path to the mod-overridden file if it exists, otherwise the base path.
    pub fn resolve_path(
        base: &Path,
        mods: &[ModManifest],
        relative_path: &str,
    ) -> PathBuf {
        // Check mods in reverse order (later mods override earlier)
        for m in mods.iter().rev() {
            let mod_file = m.path.join(relative_path);
            if mod_file.exists() {
                return mod_file;
            }
        }
        base.join(relative_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_order_sorting() {
        let mods = vec![
            ModManifest {
                name: "B Mod".into(),
                id: "b-mod".into(),
                version: "1.0.0".into(),
                author: "Test".into(),
                description: "".into(),
                load_order: 200,
                dependencies: vec![],
                path: PathBuf::new(),
            },
            ModManifest {
                name: "A Mod".into(),
                id: "a-mod".into(),
                version: "1.0.0".into(),
                author: "Test".into(),
                description: "".into(),
                load_order: 100,
                dependencies: vec![],
                path: PathBuf::new(),
            },
        ];
        let order = ModLoader::load_order(&mods);
        assert_eq!(order, vec!["a-mod", "b-mod"]);
    }

    #[test]
    fn test_resolve_path_no_mods() {
        let base = Path::new("/data");
        let mods: Vec<ModManifest> = vec![];
        let resolved = ModLoader::resolve_path(base, &mods, "items.csv");
        assert_eq!(resolved, PathBuf::from("/data/items.csv"));
    }
}
