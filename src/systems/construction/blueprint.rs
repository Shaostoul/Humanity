//! Hierarchical blueprint system — save/load building designs.
//!
//! Blueprints stored as RON files in `data/blueprints/`.

use serde::{Deserialize, Serialize};

/// A saved building blueprint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blueprint {
    pub name: String,
    pub author: String,
    // TODO: hierarchical part list, transform tree
}

impl Blueprint {
    pub fn new(name: String) -> Self {
        Self {
            name,
            author: String::new(),
        }
    }
}
