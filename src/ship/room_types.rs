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

use crate::ship::fibonacci::RoomInfo;
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
            .map(|t| t.actions.iter().map(|a| self.action_label(a)).collect())
            .unwrap_or_default()
    }

    /// One action id -> its catalog label (the id itself when the catalog has no entry).
    pub fn action_label(&self, action_id: &str) -> String {
        self.actions.get(action_id).map(|d| d.label.clone()).unwrap_or_else(|| action_id.to_string())
    }

    /// Look up a room TYPE by its rooms.ron key. None also logs a warning: a `Zone::room_type`
    /// that matches no entry is a typo in the data, and a silent blank would hide it.
    pub fn lookup_type(&self, room_type: &str) -> Option<&RoomTypeDef> {
        let def = self.types.get(room_type);
        if def.is_none() {
            log::warn!(
                "room_types: room_type '{room_type}' is not a data/rooms.ron key; the room gets no purpose or actions"
            );
        }
        def
    }

    /// The FUNCTION of one detected room, ready for the HUD's "you are in ..." surface: display
    /// name, purpose, action labels and access class. Joins on `RoomInfo::type_key` (the covering
    /// zone's `room_type` when set, else the room id, which is how the legacy fibonacci ids join).
    /// A zone-declared room_type that is unknown warns (see `lookup_type`); an anonymous "room_N"
    /// or generated "corridor_N" id is expected to have no entry and stays quiet. Fallbacks: the
    /// zone label as the name (then the id), empty purpose and actions, private access.
    pub fn function_for(&self, room: &RoomInfo) -> RoomFunction {
        let key = room.type_key();
        let def = if room.room_type.is_some() { self.lookup_type(key) } else { self.types.get(key) };
        RoomFunction {
            display_name: def.map(|t| t.name.clone()).unwrap_or_else(|| {
                if room.label.is_empty() {
                    room.id.clone()
                } else {
                    room.label.clone()
                }
            }),
            purpose: def.map(|t| t.purpose.clone()).unwrap_or_default(),
            actions: def.map(|t| t.actions.iter().map(|a| self.action_label(a)).collect()).unwrap_or_default(),
            access: def.map(|t| t.access.clone()).unwrap_or_else(default_access),
        }
    }
}

/// What a detected room is FOR, as the HUD shows it (see `RoomTypeRegistry::function_for`).
#[derive(Debug, Clone, Default)]
pub struct RoomFunction {
    pub display_name: String,
    pub purpose: String,
    /// Action LABELS (already resolved through the catalog), in rooms.ron order.
    pub actions: Vec<String>,
    pub access: String,
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

    /// The zone -> room_type -> rooms.ron join (console-room increment). A zone naming "computer"
    /// yields exactly that entry's action ids; a zone naming a key rooms.ron does not have yields
    /// none (and warns, see `lookup_type`); a zone with no room_type, or an unknown zone id, yields
    /// none quietly.
    #[test]
    fn a_zone_room_type_joins_to_its_rooms_ron_actions_and_an_unknown_one_yields_none() {
        use crate::ship::home_structure::{HomeStructure, Zone};
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let reg = RoomTypeRegistry::load(&dir);
        let mut h: HomeStructure = ron::from_str("(width: 10.0, depth: 10.0, height: 3.0)").expect("parses");
        let zone = |id: &str, room_type: Option<&str>| Zone {
            id: id.to_string(),
            type_id: "console_room".to_string(),
            origin: (0.0, 0.0, 0.0),
            size: (3.0, 3.0, 3.0),
            label: String::new(),
            room_type: room_type.map(str::to_string),
        };
        h.zones.push(zone("z-computer", Some("computer")));
        h.zones.push(zone("z-typo", Some("no_such_room_type")));
        h.zones.push(zone("z-bare", None));

        // The shipped computer entry's own action list, in its order; if the data changes this
        // expectation changes with it (the join must hand back exactly what rooms.ron says).
        let got = h.room_actions_for("z-computer", &reg);
        assert_eq!(got, vec!["use_terminal", "manage_saves", "review_tasks"], "the computer entry's actions");
        for a in &got {
            assert!(reg.actions.contains_key(a), "action id '{a}' exists in room_actions.ron");
        }
        assert!(h.room_actions_for("z-typo", &reg).is_empty(), "an unknown room_type yields no actions");
        assert!(reg.lookup_type("no_such_room_type").is_none(), "and the lookup itself is None (warned)");
        assert!(h.room_actions_for("z-bare", &reg).is_empty(), "a zone without a room_type yields none");
        assert!(h.room_actions_for("no-such-zone", &reg).is_empty(), "an unknown zone id yields none");
    }

    /// `function_for` is the HUD's view of the same join: a room_type resolves name / purpose /
    /// action labels from rooms.ron; a zone-named room with no room_type shows its zone label; an
    /// anonymous room shows its id.
    #[test]
    fn function_for_resolves_the_room_type_and_falls_back_to_label_then_id() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let reg = RoomTypeRegistry::load(&dir);
        let room = |id: &str, room_type: Option<&str>, label: &str| RoomInfo {
            id: id.to_string(),
            center: glam::Vec3::ZERO,
            dimensions: glam::Vec3::ONE,
            is_hologram_room: false,
            is_spawn_room: false,
            room_type: room_type.map(str::to_string),
            label: label.to_string(),
        };
        let f = reg.function_for(&room("console-room", Some("console_room"), "Console room"));
        assert_eq!(f.display_name, "Console room");
        assert!(f.purpose.to_lowercase().contains("workstation"), "purpose from rooms.ron: {}", f.purpose);
        assert!(f.actions.contains(&"Use Terminal".to_string()), "action LABELS, not ids: {:?}", f.actions);
        let f = reg.function_for(&room("room-entry", None, "Entry"));
        assert_eq!(f.display_name, "Entry", "no room_type -> the zone label");
        assert!(f.actions.is_empty() && f.purpose.is_empty());
        let f = reg.function_for(&room("room_7", None, ""));
        assert_eq!(f.display_name, "room_7", "no zone at all -> the id");
        // A legacy fibonacci id is itself the key (unchanged behaviour).
        let f = reg.function_for(&room("kitchen", None, ""));
        assert_eq!(f.display_name, "Kitchen");
    }
}
