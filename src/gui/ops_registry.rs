//! Server-owner action registry loader: `data/admin/ops_registry.json`, the
//! data-driven map of EVERY admin/owner/moderator action and where it lives
//! (this app, a /chat-command, a config file, or the VPS shell). The Server
//! Settings "Admin map" section renders it so a first-time server owner
//! always knows what is possible without leaving the app, and an AI can
//! enumerate the whole action surface from the one JSON. Disk-first (so an
//! operator can edit it live), embedded fallback for a zero-file install -
//! the same pattern as the other registries. See docs/design/in-app-ops.md.

use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct OpsAction {
    pub name: String,
    /// One plain-English sentence a first-time server owner understands.
    pub what: String,
    /// Which surface: native-app | web-page | chat-command | config-file | vps-shell.
    #[serde(rename = "where")]
    pub surface: String,
    /// The exact invocation: page + control, command syntax, file + key, or recipe.
    pub how: String,
    #[serde(default)]
    pub audience: Option<String>,
    #[serde(default)]
    pub also_available: Option<Vec<String>>,
}

#[derive(Deserialize, Clone)]
pub struct OpsPlanned {
    pub name: String,
    pub today: String,
    pub proposal: String,
    pub effort: String,
}

#[derive(Deserialize, Clone)]
pub struct OpsVpsOnly {
    pub name: String,
    pub reason: String,
}

#[derive(Deserialize, Clone)]
pub struct OpsRegistry {
    /// Surface id -> one-line description shown as the group header hint.
    pub surfaces: std::collections::BTreeMap<String, String>,
    pub actions: Vec<OpsAction>,
    #[serde(default)]
    pub planned: Vec<OpsPlanned>,
    #[serde(default)]
    pub vps_only: Vec<OpsVpsOnly>,
}

/// Stable render order for the surface groups: in-app first (what you can do
/// HERE), the shell last (what genuinely needs SSH).
pub const SURFACE_ORDER: [&str; 5] =
    ["native-app", "web-page", "chat-command", "config-file", "vps-shell"];

/// Human labels for the group headers.
pub fn surface_label(surface: &str) -> &'static str {
    match surface {
        "native-app" => "In this app",
        "web-page" => "On the website",
        "chat-command" => "Chat /commands",
        "config-file" => "Server config file",
        "vps-shell" => "VPS shell only (needs SSH)",
        _ => "Other",
    }
}

/// The registry, parsed once. Disk first (data/admin/ops_registry.json under
/// the resolved data dir), embedded copy as the fallback.
pub fn ops_registry() -> &'static OpsRegistry {
    static REG: std::sync::OnceLock<OpsRegistry> = std::sync::OnceLock::new();
    REG.get_or_init(|| {
        let disk = crate::data_dir().join("admin/ops_registry.json");
        let text = std::fs::read_to_string(&disk).unwrap_or_else(|e| {
            crate::embedded_data::note_builtin_copy(
                "admin/ops_registry.json",
                format_args!("{} could not be read ({e})", disk.display()),
            );
            include_str!("../../data/admin/ops_registry.json").to_string()
        });
        match serde_json::from_str::<OpsRegistry>(&text) {
            Ok(r) => r,
            Err(e) => {
                log::error!("ops_registry.json parse error ({e}); Admin map will be empty");
                OpsRegistry {
                    surfaces: Default::default(),
                    actions: Vec::new(),
                    planned: Vec::new(),
                    vps_only: Vec::new(),
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_registry_parses_and_covers_every_surface() {
        let r: OpsRegistry =
            serde_json::from_str(include_str!("../../data/admin/ops_registry.json"))
                .expect("data/admin/ops_registry.json must parse into OpsRegistry");
        assert!(r.actions.len() >= 50, "registry lost actions: {}", r.actions.len());
        assert!(!r.vps_only.is_empty(), "vps_only section missing");
        for a in &r.actions {
            assert!(
                SURFACE_ORDER.contains(&a.surface.as_str()),
                "action {:?} has unknown surface {:?}",
                a.name,
                a.surface
            );
        }
    }

    /// Where an action's "how" points at code by a source file and a function name
    /// (`src/gui/pages/game_admin.rs draw_plot_release`), that file defines that function.
    /// A file that merely exists is not enough: the third review of ship homes 1b found the
    /// plot-release entry naming msg_handlers.rs for a handler that lives in home_plots.rs,
    /// which sends an admin or an AI to a file that has never held it. A name counts as a
    /// function when it follows the path after a space and is snake_case with an underscore
    /// (plain words after a path, "src/relay/relay.rs defaults", and a name in brackets, "src/
    /// lib.rs:562 (env_logger)", are prose).
    ///
    /// Seen red 2026-10-03 on the a504c5cd9 registry: "Release a player's plot on the ship:
    /// src/relay/handlers/msg_handlers.rs has no fn handle_game_release_plot".
    #[test]
    fn every_code_pointer_names_a_function_that_file_defines() {
        let r: OpsRegistry = serde_json::from_str(include_str!("../../data/admin/ops_registry.json")).expect("the registry parses");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut bad = Vec::new();
        let mut checked = 0;
        for a in &r.actions {
            let words: Vec<&str> = a.how.split_whitespace().collect();
            for pair in words.windows(2) {
                let file = pair[0].trim_start_matches('(').split(':').next().unwrap_or("");
                let name = pair[1].trim_end_matches(|c: char| !(c.is_ascii_alphanumeric() || c == '_'));
                let is_fn = name.contains('_') && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
                if !(file.starts_with("src/") && file.ends_with(".rs") && is_fn) {
                    continue;
                }
                checked += 1;
                let text = std::fs::read_to_string(root.join(file)).unwrap_or_default();
                if !text.contains(&format!("fn {name}")) {
                    bad.push(format!("{}: {file} has no fn {name}", a.name));
                }
            }
        }
        assert!(checked > 0, "no code pointer was found to check");
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }
}
