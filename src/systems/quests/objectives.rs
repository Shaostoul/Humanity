//! Quest objective types — what the player needs to accomplish for each step.

use serde::{Deserialize, Serialize};

/// A single step in a quest chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestStep {
    /// Human-readable description of what to do (e.g., "Gather 10 wood").
    /// It is the HUD's quest line: shown as "<text> (1/8)" and cut at 64
    /// characters (src/gui/pages/hud.rs). It may name what the player's own
    /// front door opens on with `{front_door_opens_on}` (`quests::step_text`).
    pub description: String,
    /// The objective that must be satisfied to complete this step.
    pub objective: QuestObjective,
}

/// Specific objective types that the quest system can evaluate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuestObjective {
    /// Collect items — checked against player inventory.
    Gather { item_id: String, quantity: u32 },
    /// Craft items — tracked via progress counter from crafting system.
    Craft { recipe_id: String, quantity: u32 },
    /// Make items by ANY recipe that produces them (2026-10-02): counts the
    /// units made, whichever way. Smelting iron with coal and with graphite
    /// are two recipes; "make an iron ingot" is one goal, and a `Craft` step
    /// naming one recipe left the other route uncounted. The crafting system
    /// reports each unit it produces as a `make_<item_id>` event.
    Make { item_id: String, quantity: u32 },
    /// Harvest crops — tracked via progress counter from farming system.
    Harvest { crop_id: String, quantity: u32 },
    /// Build a structure — tracked via progress counter from construction system.
    Build { blueprint_id: String },
    /// Be at a destination (data/entities/destinations.ron). Complete while
    /// the player stands in it AND this is the step they are on (2026-10-04):
    /// walking past a place before the step that asks for it does not count,
    /// so the opening's last step, out of your own front door, ends where the
    /// player is, not wherever they happened to pass the door earlier.
    Travel { destination: String },
    /// Talk to an NPC — tracked via progress counter from interaction system.
    Talk { npc_id: String },
    /// Eat (2026-10-04, the opening): the food system reports each item the
    /// player actually eats as `eat_<item_id>` (FoodSystem, the consume
    /// channel the Eat button feeds), so a click on food the player does not
    /// have counts nothing. `item_id` None counts any food, Some only that item.
    /// Drinking is not eating.
    Eat {
        #[serde(default)]
        item_id: Option<String>,
        quantity: u32,
    },
    /// Plant (2026-10-04, the opening): the farming system reports each crop
    /// the player plants, from a seed in the backpack, a tower or a bed, as
    /// `plant_<plant_id>`. A crop the showcase garden or a save puts there is
    /// not planted by the player and counts nothing. `crop_id` None counts any
    /// crop, Some only that one (a plants.csv id).
    Plant {
        #[serde(default)]
        crop_id: Option<String>,
        quantity: u32,
    },
    /// Open a view of the game (2026-10-04, the opening): the interface
    /// reports each view as `view_<view>` the moment it comes on screen.
    /// "vitals" is the Inventory page's Status section, every vital with its
    /// numbers (src/gui/pages/inventory.rs).
    View { view: String },
}
