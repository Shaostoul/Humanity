//! Room-type registry: the single source of truth for what a room is FOR.
//!
//! Loads `data/rooms.ron` (room id -> name, purpose, equipment, tags, in-room actions)
//! and `data/rooms/room_actions.ron` (action id -> label + which page/action it opens).
//! The live walkable world joins each room's id to this registry at load so a room finally
//! KNOWS its function, instead of the purpose text living only on the Home design page.
//!
//! Pure data (serde) so it compiles under every feature set. New fields are
//! `#[serde(default)]` so the existing rooms.ron entries (which predate them) still parse.
//! NOTE: distinct from `ship::layout::RoomDef` (the ship-layout schema); this parses the
//! homestead room-TYPE catalog.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// One room type from `data/rooms.ron`. Only the fields the gameplay layer needs are
/// declared; serde ignores the rest (color/material/power/sound/size), so this stays a
/// thin view over the richer data file.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct RoomTypeDef {
    pub name: String,
    pub purpose: String,
    #[serde(default)]
    pub equipment: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// In-room action ids (resolve to labels/pages via the action catalog). NEW (v0.439):
    /// serde default so the existing entries without it still parse.
    #[serde(default)]
    pub actions: Vec<String>,
    /// Visibility to other players: "private" | "shared" | "public". Reserved for the
    /// home-visiting / multiplayer layer; private rooms (bathroom, bedroom) stay unseen.
    #[serde(default = "default_access")]
    pub access: String,
}

fn default_access() -> String {
    "private".to_string()
}

/// One in-room action from `data/rooms/room_actions.ron`: a label + the page/action it
/// opens. `page` is unwired in the keystone slice (shown as text); later increments route
/// the walk-up [E] surface to it.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct RoomActionDef {
    pub label: String,
    #[serde(default)]
    pub page: String,
    #[serde(default)]
    pub filter: String,
}

/// The joined registry of room types + actions.
#[derive(Debug, Clone, Default)]
pub struct RoomTypeRegistry {
    pub types: HashMap<String, RoomTypeDef>,
    pub actions: HashMap<String, RoomActionDef>,
}

impl RoomTypeRegistry {
    /// Load from `data/rooms.ron` + `data/rooms/room_actions.ron`, falling back to empty
    /// (with a warning) on a missing or invalid file so the caller degrades gracefully.
    pub fn load(data_dir: &Path) -> Self {
        let types = std::fs::read_to_string(data_dir.join("rooms.ron"))
            .ok()
            .and_then(|t| match ron::from_str::<HashMap<String, RoomTypeDef>>(&t) {
                Ok(m) => Some(m),
                Err(e) => {
                    log::warn!("room_types: failed to parse rooms.ron: {e}");
                    None
                }
            })
            .unwrap_or_default();
        let actions = std::fs::read_to_string(data_dir.join("rooms").join("room_actions.ron"))
            .ok()
            .and_then(|t| ron::from_str::<HashMap<String, RoomActionDef>>(&t).ok())
            .unwrap_or_default();
        Self { types, actions }
    }

    /// Display name for a room id (falls back to the id itself).
    pub fn name(&self, id: &str) -> String {
        self.types.get(id).map(|t| t.name.clone()).unwrap_or_else(|| id.to_string())
    }

    /// Purpose text for a room id (empty if unknown).
    pub fn purpose(&self, id: &str) -> String {
        self.types.get(id).map(|t| t.purpose.clone()).unwrap_or_default()
    }

    /// Access class for a room id ("private" if unknown).
    pub fn access(&self, id: &str) -> String {
        self.types.get(id).map(|t| t.access.clone()).unwrap_or_else(default_access)
    }

    /// Human labels for a room's in-room actions, resolved through the action catalog
    /// (an unknown action id falls back to the id itself).
    pub fn action_labels(&self, id: &str) -> Vec<String> {
        self.types
            .get(id)
            .map(|t| {
                t.actions
                    .iter()
                    .map(|a| self.actions.get(a).map(|d| d.label.clone()).unwrap_or_else(|| a.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rooms_and_joins_actions() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let reg = RoomTypeRegistry::load(&dir);
        // The shipped rooms.ron should parse and contain the homestead rooms.
        assert!(!reg.types.is_empty(), "rooms.ron should parse with entries");
        assert_eq!(reg.name("respawner"), "Respawn Chamber");
        assert!(reg.purpose("kitchen").to_lowercase().contains("food"), "kitchen purpose mentions food");
        assert_eq!(reg.access("bedroom"), "private", "rooms default to private access");
        // Actions declared on a room resolve to labels via the catalog (or fall back to id).
        let labels = reg.action_labels("kitchen");
        // If kitchen declares actions, every label is non-empty.
        for l in &labels {
            assert!(!l.is_empty());
        }
    }
}
