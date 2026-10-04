//! Economy system — credits, trading, age-based starting balance.
//!
//! Core mechanic: every player starts with 1 credit per day they've been alive.
//! A 36-year-old starts with ~13,149 credits. A teenager with ~5,475.
//! Passive income: 1 credit per game day just for existing (v0.747: REAL —
//! paid into the player's Wallet component; was a TODO log line).
//!
//! Data: data/economy.ron (formula, earning rates, trade fees)
//!       data/trade_goods.ron (250 item base values -> TradeGoodsRegistry; every id is an item, see the test every_shipped_trade_good_is_an_item)

pub mod fleet;

use crate::hot_reload::data_store::DataStore;
use crate::ecs::systems::System;
use serde::Deserialize;
use std::collections::HashMap;

/// One tradeable good's base value (a data/trade_goods.ron row). NPC prices
/// derive from base_value per the file's own formulas: vendors SELL at 1.25x
/// (25 percent markup) and BUY at 0.5x (they need margin).
#[derive(Debug, Clone, Deserialize)]
pub struct TradeGood {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub category: String,
    pub base_value: u32,
    #[serde(default)]
    pub weight_kg: f32,
    #[serde(default)]
    pub description: String,
}

/// All trade goods keyed by item id. Lives in the DataStore under
/// `"trade_goods_registry"` (v0.747, closure ladder rung 3).
#[derive(Debug, Default)]
pub struct TradeGoodsRegistry {
    pub goods: HashMap<String, TradeGood>,
}

impl TradeGoodsRegistry {
    pub fn from_ron(bytes: &[u8]) -> Result<Self, String> {
        let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
        let rows: Vec<TradeGood> = ron::from_str(text).map_err(|e| e.to_string())?;
        let mut goods = HashMap::new();
        for row in rows {
            goods.insert(row.id.clone(), row);
        }
        Ok(Self { goods })
    }

    pub fn get(&self, id: &str) -> Option<&TradeGood> {
        self.goods.get(id)
    }

    /// What an NPC vendor CHARGES the player (base x 1.25, rounded up, min 1).
    pub fn vendor_sell_price(&self, id: &str) -> Option<i64> {
        self.get(id).map(|g| ((g.base_value as f64 * 1.25).ceil() as i64).max(1))
    }

    /// What an NPC vendor PAYS the player (base x 0.5, rounded down).
    pub fn vendor_buy_price(&self, id: &str) -> Option<i64> {
        self.get(id).map(|g| (g.base_value as f64 * 0.5).floor() as i64)
    }

    pub fn len(&self) -> usize {
        self.goods.len()
    }
}

// ── Equipment (v0.750, closure ladder rung 8 / progression doc Part 3) ──
// Lives beside the trade registry because both are sparse per-item stat
// tables joined on items.csv ids.

/// One data/equipment.csv row: what an item DOES when worn. Slots reference
/// data/inventory/equipment_slots.json; stat_modifiers speak the
/// status_effects.csv `stat:value:op` grammar (ONE modifier grammar, ever).
#[derive(Debug, Clone, Deserialize)]
pub struct EquipmentDef {
    pub id: String,
    pub slot: String,
    /// Thermal insulation in clo (2026-09-27), added to the everyday outfit
    /// by the body heat model (`systems::body_heat`). Sources per row in the
    /// csv header.
    #[serde(default)]
    pub clo: f32,
    #[serde(default)]
    pub armor_kinetic: f32,
    #[serde(default)]
    pub armor_thermal: f32,
    #[serde(default)]
    pub armor_energy: f32,
    #[serde(default)]
    pub armor_chemical: f32,
    #[serde(default)]
    pub armor_radiation: f32,
    #[serde(default)]
    pub damage: f32,
    #[serde(default)]
    pub damage_type: String,
    #[serde(default)]
    pub range_m: f32,
    #[serde(default)]
    pub stat_modifiers: String,
    #[serde(default)]
    pub description: String,
}

/// All equipment stats keyed by item id. DataStore: `"equipment_registry"`.
#[derive(Debug, Default)]
pub struct EquipmentRegistry {
    pub defs: HashMap<String, EquipmentDef>,
}

impl EquipmentRegistry {
    pub fn from_csv(data: &[u8]) -> Result<Self, String> {
        let rows: Vec<EquipmentDef> = crate::assets::loader::parse_csv(data)?;
        let mut defs = HashMap::new();
        for row in rows {
            defs.insert(row.id.clone(), row);
        }
        Ok(Self { defs })
    }

    pub fn get(&self, id: &str) -> Option<&EquipmentDef> {
        self.defs.get(id)
    }

    /// Fold every WORN item's `stat_modifiers` for one stat — the same
    /// multiply/add math as StatusEffectRegistry::net_stat_multiplier, so
    /// gear and buffs never diverge in grammar. `worn` is the Outfit map's
    /// item ids.
    pub fn net_stat_multiplier<'a>(
        &self,
        worn: impl IntoIterator<Item = &'a str>,
        stat: &str,
    ) -> f32 {
        let mut mult = 1.0_f32;
        for id in worn {
            if let Some(def) = self.get(id) {
                for m in def.stat_modifiers.split('|') {
                    let mut parts = m.split(':');
                    let (Some(s), Some(v), Some(op)) =
                        (parts.next(), parts.next(), parts.next())
                    else {
                        continue;
                    };
                    if s != stat {
                        continue;
                    }
                    let Ok(value) = v.parse::<f32>() else { continue };
                    match op {
                        "multiply" => mult *= value,
                        "add" => mult += value,
                        _ => {}
                    }
                }
            }
        }
        mult.max(0.0)
    }

    /// Sum worn items' armor columns into the Armor component's
    /// per-damage-type resistance map (keys match CombatSystem's damage_type
    /// strings), capped at 0.85 per type so no outfit is ever immune. (v0.761)
    pub fn armor_from_worn<'a>(
        &self,
        worn: impl IntoIterator<Item = &'a str>,
    ) -> HashMap<String, f32> {
        let mut map: HashMap<String, f32> = HashMap::new();
        for id in worn {
            if let Some(def) = self.get(id) {
                for (key, v) in [
                    ("kinetic", def.armor_kinetic),
                    ("thermal", def.armor_thermal),
                    ("energy", def.armor_energy),
                    ("chemical", def.armor_chemical),
                    ("radiation", def.armor_radiation),
                ] {
                    if v > 0.0 {
                        *map.entry(key.to_string()).or_insert(0.0) += v;
                    }
                }
            }
        }
        for v in map.values_mut() {
            *v = v.min(0.85);
        }
        map
    }

    /// The insulation worn items add, clo (2026-09-27): garment values
    /// summed, the method the ASHRAE and ISO garment tables are built for.
    /// Items with no row add nothing.
    pub fn clo_total<'a>(&self, worn: impl IntoIterator<Item = &'a str>) -> f32 {
        worn.into_iter().filter_map(|id| self.get(id)).map(|d| d.clo.max(0.0)).sum()
    }

    /// Sum a stat's `add` values across worn items as an ABSOLUTE bonus
    /// (carry_capacity works in kg, not a multiplier).
    pub fn stat_add_total<'a>(&self, worn: impl IntoIterator<Item = &'a str>, stat: &str) -> f32 {
        let mut total = 0.0_f32;
        for id in worn {
            if let Some(def) = self.get(id) {
                for m in def.stat_modifiers.split('|') {
                    let mut parts = m.split(':');
                    if parts.next() == Some(stat) {
                        if let (Some(v), Some("add")) = (parts.next(), parts.next()) {
                            total += v.parse::<f32>().unwrap_or(0.0);
                        }
                    }
                }
            }
        }
        total
    }
}

/// Buy `qty` of `item_id` from an NPC vendor: charges the wallet, adds the
/// items volume-gated (a full pack refuses rather than losing paid goods).
/// Pure over the borrowed parts so it is directly testable; lib.rs's vendor
/// bridge calls it. Returns a human-readable receipt or refusal.
pub fn vendor_buy(
    inv: &mut crate::systems::inventory::Inventory,
    credits: &mut i64,
    goods: &TradeGoodsRegistry,
    items: Option<&crate::systems::inventory::ItemRegistry>,
    item_id: &str,
    qty: u32,
) -> Result<String, String> {
    let price = goods
        .vendor_sell_price(item_id)
        .ok_or_else(|| format!("{item_id} is not traded here"))?;
    let total = price * qty as i64;
    if *credits < total {
        return Err(format!("Not enough credits ({total} CR needed)"));
    }
    let max_stack = items.map(|r| r.max_stack_for(item_id)).unwrap_or(99);
    let unit_vol = items.map(|r| r.volume_for(item_id)).unwrap_or(0.0);
    // Refuse on overflow BEFORE charging: paid goods must never be lost.
    let lost = inv.add_item_volume_gated(item_id, qty, max_stack, unit_vol);
    if lost > 0 {
        // Roll back what did fit.
        inv.remove_item(item_id, qty - lost);
        return Err("Not enough room in your pack".to_string());
    }
    *credits -= total;
    Ok(format!("Bought {qty}x {item_id} for {total} CR"))
}

/// Sell `qty` of `item_id` to an NPC vendor: removes the items, pays 0.5x base.
pub fn vendor_sell(
    inv: &mut crate::systems::inventory::Inventory,
    credits: &mut i64,
    goods: &TradeGoodsRegistry,
    item_id: &str,
    qty: u32,
    quality: u8,
    levels: Option<&crate::systems::crafting::quality::QualityLevels>,
) -> Result<String, String> {
    let price = goods
        .vendor_buy_price(item_id)
        .ok_or_else(|| format!("{item_id} is not traded here"))?;
    // One grade at a time, at that grade's price (2026-09-26): a good hammer
    // fetches more than a poor one, and defective goods are not bought.
    let m = levels.map_or(1.0, |l| l.price_multiplier(quality) as f64);
    if m <= 0.0 {
        return Err("The vendor will not buy defective goods: scrap or recycle them.".to_string());
    }
    let have: u32 = inv
        .slots
        .iter()
        .flatten()
        .filter(|s| s.item_id == item_id && s.quality == quality)
        .map(|s| s.quantity)
        .sum();
    if have < qty {
        return Err(format!("You only have {have}x {item_id}"));
    }
    inv.remove_graded(item_id, qty, quality);
    let total = (price as f64 * m).floor() as i64 * qty as i64;
    *credits += total;
    Ok(format!("Sold {qty}x {item_id} for {total} CR"))
}

/// Economy system: manages credits, passive income, and market pricing.
pub struct EconomySystem {
    /// Credits per day alive (from economy.ron, default 1.0)
    pub credits_per_day_alive: f32,
    /// Passive income per real-time day (default 1.0)
    pub passive_income_per_day: f32,
    /// Seconds of passive income accumulated since last payout
    passive_timer: f32,
}

impl EconomySystem {
    pub fn new() -> Self {
        Self {
            credits_per_day_alive: 1.0,
            passive_income_per_day: 1.0,
            passive_timer: 0.0,
        }
    }

    /// Test hook: put the passive-income timer `seconds` away from a payout.
    #[cfg(test)]
    fn force_payout_in(&mut self, seconds: f32) {
        self.passive_timer = crate::systems::time::EARTH_DAY_S as f32 - seconds;
    }

    /// Calculate starting credits from a birth date string (YYYY-MM-DD format).
    /// Returns floor(days_alive * credits_per_day_alive).
    pub fn calculate_starting_credits(&self, birth_date_str: &str) -> u64 {
        let parts: Vec<&str> = birth_date_str.split('-').collect();
        if parts.len() != 3 {
            return 0;
        }
        let year: i32 = parts[0].parse().unwrap_or(2000);
        let month: u32 = parts[1].parse().unwrap_or(1);
        let day: u32 = parts[2].parse().unwrap_or(1);

        // Simple days-since-epoch calculation (approximate, good enough for credits)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let now_days = now / 86400;

        // Convert birth date to approximate days since epoch
        // (rough calculation: doesn't account for all leap years perfectly)
        let birth_days = {
            let y = year as u64;
            let m = month as u64;
            let d = day as u64;
            // Days from year
            let mut total = y * 365 + y / 4 - y / 100 + y / 400;
            // Days from month (approximate)
            let month_days: [u64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
            for i in 0..(m.min(12) - 1) as usize {
                total += month_days[i];
            }
            total += d;
            // Offset to Unix epoch (Jan 1, 1970)
            total.saturating_sub(719528) // days from year 0 to 1970
        };

        let days_alive = now_days.saturating_sub(birth_days);
        (days_alive as f32 * self.credits_per_day_alive) as u64
    }
}

impl System for EconomySystem {
    fn name(&self) -> &str {
        "economy"
    }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore) {
        // Passive income (v0.747, REAL): 1 credit per GAME day into every
        // Wallet — "nobody is ever stuck at zero" (economy.ron's design
        // note). The day is the calendar's (hours in a day, from Settings) on
        // the one game clock (2026-09-27): game seconds, at the time speed.
        self.passive_timer += crate::systems::time::scaled_dt(dt, data);
        let day_seconds = data
            .get::<std::sync::Mutex<crate::systems::time::GameTime>>("game_time")
            .and_then(|m| m.lock().ok().map(|g| g.seconds_per_day()))
            .unwrap_or(crate::systems::time::EARTH_DAY_S) as f32;
        if self.passive_timer >= day_seconds {
            self.passive_timer -= day_seconds;
            let income = self.passive_income_per_day as i64;
            for (_e, wallet) in world.query_mut::<&mut crate::ecs::components::Wallet>() {
                wallet.credits += income;
            }
            log::debug!("Passive income: +{income} CR");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_starting_credits() {
        let econ = EconomySystem::new();
        // Someone born in 1988 should have roughly 13,000-14,000 credits in 2026
        let credits = econ.calculate_starting_credits("1988-01-29");
        assert!(credits > 10000, "Expected >10000 credits, got {}", credits);
        assert!(credits < 20000, "Expected <20000 credits, got {}", credits);
    }

    #[test]
    fn test_invalid_date() {
        let econ = EconomySystem::new();
        assert_eq!(econ.calculate_starting_credits("invalid"), 0);
    }

    fn shipped_goods() -> TradeGoodsRegistry {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/trade_goods.ron");
        TradeGoodsRegistry::from_ron(&std::fs::read(path).unwrap()).unwrap()
    }

    /// data/items.csv through ItemRegistry, the runtime's own loader.
    fn shipped_items() -> crate::systems::inventory::ItemRegistry {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/items.csv");
        crate::systems::inventory::ItemRegistry::from_csv(&std::fs::read(path).unwrap()).unwrap()
    }

    /// data/recipes.csv through RecipeRegistry::from_csv, the runtime's own loader.
    fn shipped_recipes() -> crate::systems::crafting::RecipeRegistry {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/recipes.csv");
        crate::systems::crafting::RecipeRegistry::from_csv(&std::fs::read(path).unwrap()).unwrap()
    }

    /// What the vendor charges for an item it stocks: a trade good that is
    /// also an items.csv item (the src/lib.rs catalog filter). None when it
    /// does not stock it.
    fn vendor_charge(
        items: &crate::systems::inventory::ItemRegistry,
        goods: &TradeGoodsRegistry,
        id: &str,
    ) -> Option<f64> {
        if items.items.contains_key(id) {
            goods.vendor_sell_price(id).map(|p| p as f64)
        } else {
            None
        }
    }

    /// What the vendor pays for a trade good (nothing for anything else).
    fn vendor_pays(goods: &TradeGoodsRegistry, id: &str) -> f64 {
        goods.vendor_buy_price(id).unwrap_or(0) as f64
    }

    /// What a recipe's inputs cost at `cheapest`; None when one has no price.
    fn inputs_cost(r: &crate::systems::crafting::Recipe, cheapest: &HashMap<String, f64>) -> Option<f64> {
        r.inputs.iter().map(|(id, q)| cheapest.get(id).map(|c| c * *q as f64)).sum()
    }

    /// The cheapest credits-to-item cost of every item a player can get for
    /// credits (BUG-145): buy it from the vendor, or craft it from cheaper
    /// inputs, selling the byproducts. Returns the recipes that have inputs
    /// (a recipe with none is gathering, not buying), sorted by id so a walk
    /// and its messages are the same on every run, and the costs. Settles in
    /// a few rounds; a cost that keeps falling means a cycle of recipes makes
    /// goods from nothing, itself a loop.
    fn cheapest_costs<'a>(
        items: &crate::systems::inventory::ItemRegistry,
        goods: &TradeGoodsRegistry,
        recipes: &'a crate::systems::crafting::RecipeRegistry,
    ) -> (Vec<&'a crate::systems::crafting::Recipe>, HashMap<String, f64>) {
        let mut book: Vec<&crate::systems::crafting::Recipe> =
            recipes.recipes.values().filter(|r| !r.inputs.is_empty()).collect();
        book.sort_by(|a, b| a.id.cmp(&b.id));
        let mut cheapest: HashMap<String, f64> = goods
            .goods
            .keys()
            .filter_map(|id| vendor_charge(items, goods, id).map(|c| (id.clone(), c)))
            .collect();
        let mut rounds = 0;
        loop {
            let mut changed = false;
            for r in &book {
                let Some(cost) = inputs_cost(r, &cheapest) else { continue };
                for (i, (out, q)) in r.outputs.iter().enumerate() {
                    if *q == 0 {
                        continue;
                    }
                    let byproducts: f64 = r
                        .outputs
                        .iter()
                        .enumerate()
                        .filter(|(j, _)| *j != i)
                        .map(|(_, (id, qq))| vendor_pays(goods, id) * *qq as f64)
                        .sum();
                    let unit = (cost - byproducts) / *q as f64;
                    if cheapest.get(out).map_or(true, |c| unit < c - 1e-9) {
                        cheapest.insert(out.clone(), unit);
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
            rounds += 1;
            assert!(rounds < 100, "item costs never settle: a cycle of recipes makes goods from nothing");
        }
        (book, cheapest)
    }

    /// v0.747 (ladder rung 3): the shipped trade_goods.ron parses and the price
    /// formulas match the file's own documentation (sell 1.25x up, buy 0.5x down).
    #[test]
    fn trade_goods_registry_parses_shipped_file_with_documented_prices() {
        let reg = shipped_goods();
        assert!(reg.len() >= 200, "expected the full catalog, got {}", reg.len());
        let iron = reg.get("iron_ore_0").expect("iron ore is traded");
        assert_eq!(iron.base_value, 5);
        assert_eq!(reg.vendor_sell_price("iron_ore_0"), Some(7)); // ceil(6.25)
        assert_eq!(reg.vendor_buy_price("iron_ore_0"), Some(2)); // floor(2.5)
        assert_eq!(reg.vendor_sell_price("nope"), None);
    }

    /// BUG-143 (2026-10-04): every trade good is an item. The vendor's catalog
    /// (built in src/lib.rs) keeps only goods whose id is ALSO an items.csv id,
    /// so a trade good that is not an item is dropped without a word and the
    /// shop never offers it. 140 of the 300 were, clay_0 among them while
    /// items.csv calls it clay_raw_0. Items are read through ItemRegistry, the
    /// runtime's own loader, so a malformed new items.csv row (which the loader
    /// skips) fails here too. Also fails on an id listed twice, since the
    /// registry keeps only the last row. Seen red on the original files:
    ///   140 trade goods in data/trade_goods.ron are not items in
    ///   data/items.csv, so the vendor never offers them: ["clay_0", "dirt_0",
    ///   "bamboo_0", "sulfur_0", "saltpeter_0", "tin_ore_0", ... "oil_fish_0"]
    #[test]
    fn every_shipped_trade_good_is_an_item() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let items = crate::systems::inventory::ItemRegistry::from_csv(
            &std::fs::read(root.join("data/items.csv")).unwrap(),
        )
        .unwrap();
        let text = std::fs::read_to_string(root.join("data/trade_goods.ron")).unwrap();
        let rows: Vec<TradeGood> = ron::from_str(&text).unwrap();
        assert!(rows.len() >= 200, "expected the full catalog, got {}", rows.len());

        let mut seen = std::collections::HashSet::new();
        let twice: Vec<&str> =
            rows.iter().map(|g| g.id.as_str()).filter(|id| !seen.insert(*id)).collect();
        assert!(
            twice.is_empty(),
            "trade goods listed twice in data/trade_goods.ron (the registry keeps only the last): {twice:?}"
        );

        let missing: Vec<&str> = rows
            .iter()
            .map(|g| g.id.as_str())
            .filter(|id| !items.items.contains_key(*id))
            .collect();
        assert!(
            missing.is_empty(),
            "{} trade goods in data/trade_goods.ron are not items in data/items.csv, so the vendor \
             never offers them: {missing:?}",
            missing.len()
        );
    }

    /// BUG-145 (2026-10-04): no recipe turns goods the vendor sells into goods
    /// it buys back for more, which would be an endless money loop at any
    /// trading post. The vendor charges `vendor_sell_price` (1.25x base,
    /// rounded up) and pays `vendor_buy_price` (0.5x base, rounded down), the
    /// same two functions `vendor_buy` and `vendor_sell` settle with, and it
    /// stocks the goods src/lib.rs puts in its catalog: trade goods that are
    /// also items.csv items. Recipes are read by RecipeRegistry::from_csv,
    /// the runtime's own loader.
    ///
    /// An input counts as obtainable when the vendor sells it OR a recipe
    /// makes it from obtainable inputs, at the cheaper of the two: a loop
    /// does not need the vendor to sell every input, since buying logs,
    /// sawing planks and building a bow from the planks loops just as well
    /// as buying the planks. A crafting step that also yields byproducts is
    /// credited with what the vendor pays for them. A recipe with no inputs
    /// is gathering, not buying, and is left out. Prices are for ungraded and
    /// standard goods; a better grade earns more (QualityLevels), which is
    /// a separate question.
    ///
    /// Seen red on the data before the fix, 24 recipes, 18 of them looping
    /// through a crafted input ("n/a": an input the vendor does not sell):
    ///   24 recipes make goods the vendor buys back for more than their inputs
    ///   cost, an endless money loop (BUG-145):
    ///   assemble_computer: inputs cost 50.88 at the cheapest (buying them all: 100), the outputs sell for 100
    ///   build_mech_heavy: inputs cost 2365.83 at the cheapest (buying them all: n/a), the outputs sell for 10000
    ///   build_spacecraft_pod: inputs cost 814.58 at the cheapest (buying them all: 876), the outputs sell for 2500
    ///   craft_bow_recurve: inputs cost 7.50 at the cheapest (buying them all: 19), the outputs sell for 12
    ///   craft_stim_pack: inputs cost 13.00 at the cheapest (buying them all: 13), the outputs sell for 85
    ///   make_wire: inputs cost 12.25 at the cheapest (buying them all: 13), the outputs sell for 15
    ///   ... (and 18 more)
    #[test]
    fn no_recipe_resells_for_more_than_its_inputs_cost() {
        let items = shipped_items();
        let goods = shipped_goods();
        let recipes = shipped_recipes();
        assert!(recipes.recipes.len() >= 300, "expected the full recipe book, got {}", recipes.recipes.len());
        let (book, cheapest) = cheapest_costs(&items, &goods, &recipes);

        let mut checked = 0;
        let mut loops = Vec::new();
        for r in &book {
            let Some(cost) = inputs_cost(r, &cheapest) else { continue };
            checked += 1;
            let sells: f64 = r.outputs.iter().map(|(id, q)| vendor_pays(&goods, id) * *q as f64).sum();
            if sells > cost + 1e-9 {
                let bought: Option<f64> =
                    r.inputs.iter().map(|(id, q)| vendor_charge(&items, &goods, id).map(|c| c * *q as f64)).sum();
                let bought = bought.map_or("n/a".to_string(), |b| format!("{b}"));
                loops.push(format!(
                    "{}: inputs cost {cost:.2} at the cheapest (buying them all: {bought}), the outputs sell for {sells}",
                    r.id
                ));
            }
        }
        // Proof the walk reached the recipe book, so a green run is not an
        // empty one (a renamed column would leave nothing to check).
        assert!(checked >= 300, "only {checked} recipes have inputs the vendor sells or can be crafted from them");
        assert!(
            loops.is_empty(),
            "{} recipes make goods the vendor buys back for more than their inputs cost, an endless \
             money loop (BUG-145):\n  {}",
            loops.len(),
            loops.join("\n  ")
        );
    }

    /// The items.csv ids whose category is "vehicle".
    fn shipped_vehicle_ids() -> std::collections::HashSet<String> {
        #[derive(serde::Deserialize)]
        struct Row {
            id: String,
            #[serde(default)]
            category: String,
        }
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/items.csv");
        let rows: Vec<Row> = crate::assets::loader::parse_csv(&std::fs::read(path).unwrap()).unwrap();
        rows.into_iter().filter(|r| r.category == "vehicle").map(|r| r.id).collect()
    }

    /// Vehicle bills of materials (2026-10-04, after BUG-145): a recipe that
    /// builds a vehicle puts about the vehicle's own weight of parts into it,
    /// up to a quarter more for offcuts, never less. The vehicle recipes used
    /// to put 38 to 203 kg of parts into vehicles of 180 kg to 50 t, so BUG-145
    /// could only stop them reselling for more than their parts by pricing a
    /// spacecraft pod under a sedan. Weights are items.csv weight_kg through
    /// ItemRegistry; a vehicle is an items.csv row of category "vehicle".
    ///
    /// Only `build_` recipes are held to it. The vehicle assembler's
    /// `assemble_` recipes belong to the vehicle-kit system
    /// (data/vehicles/kits.ron) and are not real bills yet. build_raft is
    /// exempt: items.csv lists the raft as 20 kg of bamboo, but no bamboo-pole
    /// item exists, and its six 8 kg pine logs are already a light raft for
    /// one person; that is a question for the raft's item row.
    ///
    /// Seen red on the bills before the fix, 25 of the 28 recipes it checks
    /// (the two bicycles and the hand cart passed), among them:
    ///   build_spacecraft_freighter: 203.0 kg of parts for a 50000.0 kg vehicle (x0.00)
    ///   build_spacecraft_pod: 38.6 kg of parts for a 2000.0 kg vehicle (x0.02)
    ///   build_sled: 10.5 kg of parts for a 5.0 kg vehicle (x2.10)
    #[test]
    fn vehicle_recipes_weigh_what_the_vehicle_weighs() {
        const EXEMPT: &[&str] = &["build_raft"];
        let vehicles = shipped_vehicle_ids();
        let items = shipped_items();
        let recipes = shipped_recipes();
        let mut ids: Vec<&String> = recipes.recipes.keys().collect();
        ids.sort();
        let mut checked = 0;
        let mut off = Vec::new();
        for id in ids {
            let r = &recipes.recipes[id];
            if !r.id.starts_with("build_") || EXEMPT.contains(&r.id.as_str()) {
                continue;
            }
            let Some((out, q)) = r.outputs.iter().find(|(o, _)| vehicles.contains(o)) else { continue };
            checked += 1;
            let made = items.mass_for(out) as f64 * *q as f64;
            let parts: f64 = r.inputs.iter().map(|(i, n)| items.mass_for(i) as f64 * *n as f64).sum();
            let ratio = parts / made;
            if !(1.0..=1.25).contains(&ratio) {
                off.push(format!("{}: {parts:.1} kg of parts for a {made:.1} kg vehicle (x{ratio:.2})", r.id));
            }
        }
        // The vehicle recipes are reached: 29 build_ recipes make a vehicle,
        // 28 once the raft is set aside.
        assert!(checked >= 28, "only {checked} build_ recipes make an items.csv vehicle");
        assert!(
            off.is_empty(),
            "{} vehicle recipes put more than a quarter over, or less than, the vehicle's own weight of \
             parts into it (items.csv weight_kg):\n  {}",
            off.len(),
            off.join("\n  ")
        );
    }

    /// The other side of BUG-145 for vehicles (2026-10-04): a vehicle the
    /// vendor stocks is priced at least at what its parts cost at the
    /// cheapest, so the finished machine is never sold for less than the
    /// goods it is built from (BUG-145 had to price the light mech at 2500
    /// while its parts cost 1317, and with the full bill of materials they
    /// cost 74960). With no_recipe_resells_for_more_than_its_inputs_cost this
    /// holds every recipe-built vehicle's base value between its inputs' cost
    /// and twice it. Costs come from the same cheapest walk.
    ///
    /// Seen red on the BUG-145 prices with the full bills, 11 of the 12
    /// recipes (the rowboat's 70 covered its 57), among them:
    ///   build_bicycle: base value 50, but its parts cost 91.08 at the cheapest
    ///   build_spacecraft_pod: base value 1550, but its parts cost 44424.67 at the cheapest
    ///   build_spacecraft_freighter: base value 5200, but its parts cost 406708.09 at the cheapest
    #[test]
    fn no_vehicle_sells_for_less_than_its_parts() {
        let items = shipped_items();
        let goods = shipped_goods();
        let recipes = shipped_recipes();
        let (book, cheapest) = cheapest_costs(&items, &goods, &recipes);
        let mut checked = 0;
        let mut under = Vec::new();
        for r in &book {
            let Some(cost) = inputs_cost(r, &cheapest) else { continue };
            for (out, q) in &r.outputs {
                let Some(g) = goods.get(out) else { continue };
                if g.category != "vehicle" || vendor_charge(&items, &goods, out).is_none() {
                    continue;
                }
                checked += 1;
                let worth = g.base_value as f64 * *q as f64;
                if worth < cost - 1e-9 {
                    under.push(format!(
                        "{}: base value {worth}, but its parts cost {cost:.2} at the cheapest",
                        r.id
                    ));
                }
            }
        }
        // Proof the walk reached the vehicles: 12 recipes build a vehicle the
        // vendor stocks (two bicycles, the hand cart, the motorcycle, two
        // boats, four spacecraft and two mechs).
        assert!(checked >= 12, "only {checked} recipes build a vehicle the vendor stocks");
        assert!(
            under.is_empty(),
            "{} vehicles are priced under what their parts cost, so building one costs more than \
             buying it:\n  {}",
            under.len(),
            under.join("\n  ")
        );
    }

    /// A sale is priced by grade (2026-09-26): a good hammer fetches more than
    /// an ungraded one, a defective one nothing.
    #[test]
    fn a_sale_is_priced_by_grade() {
        use crate::systems::inventory::Inventory;
        let goods = shipped_goods();
        let levels = crate::systems::crafting::quality::QualityLevels::from_ron(
            &std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/data/manufacturing.ron")).unwrap(),
        )
        .unwrap();
        let base = goods.vendor_buy_price("hammer_0").expect("the vendor buys hammers");
        let sell = |quality: u8| {
            let mut inv = Inventory::new(4);
            inv.add_item_q("hammer_0", 1, 1, quality);
            let mut credits = 0i64;
            vendor_sell(&mut inv, &mut credits, &goods, "hammer_0", 1, quality, Some(&levels)).unwrap();
            credits
        };
        assert_eq!(sell(0), base, "ungraded: the base price");
        assert_eq!(sell(4), (base as f64 * 1.5).floor() as i64, "good: 1.5x");
        // Defective: refused, and the hammer is kept.
        let mut inv = Inventory::new(4);
        inv.add_item_q("hammer_0", 1, 1, 1);
        let mut credits = 0i64;
        assert!(vendor_sell(&mut inv, &mut credits, &goods, "hammer_0", 1, 1, Some(&levels)).is_err());
        assert_eq!((inv.count_item("hammer_0"), credits), (1, 0), "nothing taken, nothing paid");
        // Selling the masterwork sells the masterwork, not the defective one beside it.
        let mut inv = Inventory::new(4);
        inv.add_item_q("hammer_0", 1, 1, 6);
        inv.add_item_q("hammer_0", 1, 1, 1);
        let mut credits = 0i64;
        vendor_sell(&mut inv, &mut credits, &goods, "hammer_0", 1, 6, Some(&levels)).unwrap();
        assert_eq!(credits, (base as f64 * 5.0).floor() as i64);
        let left: Vec<u8> = inv.slots.iter().flatten().map(|s| s.quality).collect();
        assert_eq!(left, vec![1], "the defective one is still in the pack");
    }

    /// Buying charges the wallet + lands the items; refusals (broke, full pack)
    /// change NOTHING - paid goods are never lost and refusals never charge.
    #[test]
    fn vendor_buy_and_sell_round_trip() {
        use crate::systems::inventory::Inventory;
        let goods = shipped_goods();
        let mut inv = Inventory::new(8);
        let mut credits: i64 = 20;

        // Buy 2 iron ore at 7 CR each.
        let receipt = vendor_buy(&mut inv, &mut credits, &goods, None, "iron_ore_0", 2).unwrap();
        assert!(receipt.contains("14 CR"), "{receipt}");
        assert_eq!(credits, 6);
        assert_eq!(inv.count_item("iron_ore_0"), 2);

        // Too broke for 2 more: refused, nothing changes.
        let err = vendor_buy(&mut inv, &mut credits, &goods, None, "iron_ore_0", 2).unwrap_err();
        assert!(err.contains("Not enough credits"), "{err}");
        assert_eq!(credits, 6);
        assert_eq!(inv.count_item("iron_ore_0"), 2);

        // Sell both back at 2 CR each.
        let receipt = vendor_sell(&mut inv, &mut credits, &goods, "iron_ore_0", 2, 0, None).unwrap();
        assert!(receipt.contains("4 CR"), "{receipt}");
        assert_eq!(credits, 10);
        assert_eq!(inv.count_item("iron_ore_0"), 0);

        // Selling what you don't have: refused.
        assert!(vendor_sell(&mut inv, &mut credits, &goods, "iron_ore_0", 1, 0, None).is_err());
    }

    /// v0.750 (ladder rung 8): the shipped equipment.csv parses; the stat
    /// folds match the status-effect grammar (multiply/add), and the absolute
    /// add total works for carry capacity.
    #[test]
    fn equipment_registry_parses_and_folds_stats() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/equipment.csv");
        let reg = EquipmentRegistry::from_csv(&std::fs::read(path).unwrap()).unwrap();
        assert!(reg.defs.len() >= 12, "shipped gear set, got {}", reg.defs.len());
        let coat = reg.get("coat_winter_0").expect("winter coat is gear");
        assert_eq!(coat.slot, "chest");

        // Winter kit (2026-09-27, the body heat model's clo): parka 0.70 +
        // knit hat 0.03 + winter gloves 0.05 + boots 0.08 = 0.86 clo over the
        // everyday outfit; a pickaxe insulates nothing.
        let worn = ["coat_winter_0", "hat_beanie_0", "gloves_winter_0", "boots_work_0", "pickaxe_0"];
        let clo = reg.clo_total(worn.iter().copied());
        assert!((clo - 0.86).abs() < 1e-4, "kit totals 0.86 clo, got {clo}");

        // Hiking boots multiply speed.
        let speed = reg.net_stat_multiplier(["boots_hiking_0"].iter().copied(), "speed");
        assert!((speed - 1.05).abs() < 1e-4, "boots are 1.05x, got {speed}");

        // Backpacks add carry kg.
        let carry = reg.stat_add_total(["backpack_large_0"].iter().copied(), "carry_capacity");
        assert!((carry - 25.0).abs() < 1e-4, "large pack adds 25 kg, got {carry}");
    }

    /// v0.761: the armor columns bite - a worn kit sums per-damage-type
    /// resistances into the Armor map, capped so no outfit is immune.
    #[test]
    fn armor_from_worn_sums_and_caps() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/equipment.csv");
        let reg = EquipmentRegistry::from_csv(&std::fs::read(path).unwrap()).unwrap();

        // Combat helmet + work boots: kinetic 0.45 + 0.15 = 0.60.
        let map = reg.armor_from_worn(["helmet_combat_0", "boots_work_0"].iter().copied());
        assert!((map.get("kinetic").copied().unwrap_or(0.0) - 0.60).abs() < 1e-4);

        // Winter kit resists thermal hits: coat 0.35 + beanie 0.05 + gloves 0.10.
        let map = reg.armor_from_worn(
            ["coat_winter_0", "hat_beanie_0", "gloves_winter_0"].iter().copied(),
        );
        assert!((map.get("thermal").copied().unwrap_or(0.0) - 0.50).abs() < 1e-4);

        // Cap: stacking the same big piece many times never exceeds 0.85.
        let stack = ["helmet_combat_0"; 5];
        let map = reg.armor_from_worn(stack.iter().copied());
        assert!(map.get("kinetic").copied().unwrap_or(0.0) <= 0.85);

        // Cosmetics with no gear row contribute nothing.
        let map = reg.armor_from_worn(["some_cosmetic_scarf"].iter().copied());
        assert!(map.is_empty());
    }

    /// v0.747: passive income is REAL - a game-day boundary pays 1 CR into
    /// every Wallet ("nobody is ever stuck at zero").
    #[test]
    fn passive_income_pays_the_wallet_each_game_day() {
        use crate::ecs::components::Wallet;
        use crate::ecs::systems::System;
        let mut world = hecs::World::new();
        let e = world.spawn((Wallet { credits: 0 },));
        let data = DataStore::new();
        let mut sys = EconomySystem::new();
        sys.force_payout_in(1.0);
        sys.tick(&mut world, 0.5, &data); // not yet
        assert_eq!(world.get::<&Wallet>(e).unwrap().credits, 0);
        sys.tick(&mut world, 1.0, &data); // crosses the day boundary
        assert_eq!(world.get::<&Wallet>(e).unwrap().credits, 1);
    }
}
