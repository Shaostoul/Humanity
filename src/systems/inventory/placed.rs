//! One item sitting in a container of the home's places tree: the item pool
//! the Inventory page organizes and the world save persists.
//!
//! It lives here, not in `src/gui`, so the save format (`persistence`)
//! compiles into the relay as well as the desktop app (2026-09-28, the first
//! rung of multiplayer in docs/design/playable-assessment-2026-09-19.md).
//! The Inventory page builds the pool (`gui::flatten_placed_items`).

/// One item placed in a container, for the organize-layer inventory (operator
/// 2026-06-22: "one item pool; each item records WHICH container it's in", and
/// transfer = move it between containers). `container` is the container's PATH in the
/// places tree (e.g. "1/0/0"), so a transfer is just changing this string. Seeded from
/// the places spine at load; serializable so a save can persist transfers. The live
/// backpack is NOT in this pool (its items come from the ECS) until the ECS-boundary
/// transfer lands.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlacedItem {
    /// Item id (resolves against items.csv) OR a descriptive label for seed items.
    pub key: String,
    /// Display name (item name if `key` is an id; else the label).
    pub name: String,
    pub qty: u32,
    /// Container PATH in the places tree this item currently sits in.
    pub container: String,
    /// Uses worn off it (a tool, 2026-09-26): it keeps its wear in storage,
    /// so putting a worn tool away and taking it back does not renew it.
    #[serde(default)]
    pub wear: u32,
    /// Grade of a crafted durable good (0 = ungraded), kept through storage.
    #[serde(default)]
    pub quality: u8,
}
