//! Rebindable key/button bindings loaded from `config/bindings.toml`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A named game action (e.g., "move_forward", "jump", "interact").
pub type Action = String;

/// Input binding configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingConfig {
    /// Map from action name to key/button string.
    pub bindings: HashMap<Action, String>,
}

impl BindingConfig {
    /// Load bindings from a TOML file path.
    pub fn load(_path: &str) -> Self {
        // TODO: read and parse TOML file
        Self {
            bindings: HashMap::new(),
        }
    }
}
