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
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
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
    /// How long it has aged, for food (2026-10-04, first-hour audit S6), in game seconds at
    /// room temperature, as a backpack stack's (`ItemStack::age_s`): stored food spoils too
    /// (`age_food`), and it keeps its age going in and out of storage and through a save.
    /// 0 in a save from before it.
    #[serde(default)]
    pub age_s: f64,
}

/// Age the FOOD in home storage by `secs` (2026-10-04, first-hour audit S6): the game seconds
/// at room temperature the FoodSystem counted for the home's air since the last call
/// (`food::STORAGE_AGING_KEY`, put on the pool by `engine::stock_piles::age_home_storage`).
/// `is_food` says which items are food; nothing else ages. Before this only a backpack aged,
/// so the Barn's food never spoiled.
pub fn age_food(pool: &mut [PlacedItem], secs: f64, is_food: impl Fn(&str) -> bool) {
    if secs.is_nan() || secs <= 0.0 {
        return;
    }
    for p in pool.iter_mut().filter(|p| is_food(&p.key)) {
        p.age_s += secs;
    }
}

/// The container at `path`, or a container inside it ("built:3" holds
/// "built:3" and "built:3/0", never "built:30").
fn under(container: &str, path: &str) -> bool {
    container == path || container.strip_prefix(path).is_some_and(|rest| rest.starts_with('/'))
}

/// The home's storage as item -> count: every placed item except those in
/// the containers under `elsewhere`, which are the player's but not in the
/// home (a chest built on a planet holds what is on that planet, BUG-147).
/// The automated machines, the build menu and hand crafts all count this.
pub fn stock_counts(pool: &[PlacedItem], elsewhere: &[String]) -> std::collections::HashMap<String, u32> {
    let mut stock = std::collections::HashMap::new();
    for p in pool.iter().filter(|p| !elsewhere.iter().any(|e| under(&p.container, e))) {
        *stock.entry(p.key.clone()).or_insert(0) += p.qty;
    }
    stock
}

/// Take out of the pool what the systems used from home storage in a tick:
/// for each item, the drop from `before` to `after`, from its placed stacks
/// in pool order, never from a container under `elsewhere` (those were not
/// counted, so nothing was used from them). Emptied stacks go. Returns true
/// when anything was taken.
pub fn take_consumed(
    pool: &mut Vec<PlacedItem>,
    before: &std::collections::HashMap<String, u32>,
    after: &std::collections::HashMap<String, u32>,
    elsewhere: &[String],
) -> bool {
    let mut changed = false;
    for (id, before_qty) in before {
        let mut deficit = before_qty.saturating_sub(after.get(id).copied().unwrap_or(0));
        for p in pool.iter_mut() {
            if deficit == 0 {
                break;
            }
            if p.key == *id && !elsewhere.iter().any(|e| under(&p.container, e)) {
                let take = p.qty.min(deficit);
                p.qty -= take;
                deficit -= take;
                changed = true;
            }
        }
    }
    if changed {
        pool.retain(|p| p.qty > 0);
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(key: &str, qty: u32, container: &str) -> PlacedItem {
        PlacedItem { key: key.into(), name: key.into(), qty, container: container.into(), ..Default::default() }
    }

    /// A chest built on a planet is the player's, but it is not in the home:
    /// its iron is not home storage, and what the home's crafts and machines
    /// use never comes out of it. Seen red against `stock_counts` and
    /// `take_consumed` ignoring `elsewhere`: "left: Some(17), right: Some(5)".
    #[test]
    fn a_chest_built_on_a_planet_is_not_home_storage() {
        let away = vec!["built:3".to_string()];
        let mut pool = vec![
            item("iron_0", 10, "built:3"),
            item("iron_0", 2, "built:3/0"),
            item("iron_0", 4, "1/0"),
            item("iron_0", 1, "built:30"),
        ];
        let before = stock_counts(&pool, &away);
        assert_eq!(before.get("iron_0").copied(), Some(5), "the barn's 4 and the home chest's 1");
        let mut after = before.clone();
        after.insert("iron_0".into(), 0);
        assert!(take_consumed(&mut pool, &before, &after, &away));
        let left: Vec<(String, u32)> = pool.iter().map(|p| (p.container.clone(), p.qty)).collect();
        assert_eq!(left, vec![("built:3".into(), 10), ("built:3/0".into(), 2)], "only the home's iron was used");
    }
}
