//! Economy system — credits, trading, age-based starting balance.
//!
//! Core mechanic: every player starts with 1 credit per day they've been alive.
//! A 36-year-old starts with ~13,149 credits. A teenager with ~5,475.
//! Passive income: 1 credit per game day just for existing (v0.747: REAL —
//! paid into the player's Wallet component; was a TODO log line).
//!
//! Data: data/economy.ron (formula, earning rates, trade fees)
//!       data/trade_goods.ron (255 item base values -> TradeGoodsRegistry)

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
) -> Result<String, String> {
    let price = goods
        .vendor_buy_price(item_id)
        .ok_or_else(|| format!("{item_id} is not traded here"))?;
    let have = inv.count_item(item_id);
    if have < qty {
        return Err(format!("You only have {have}x {item_id}"));
    }
    inv.remove_item(item_id, qty);
    let total = price * qty as i64;
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
        self.passive_timer = crate::systems::time::SECONDS_PER_DAY as f32 - seconds;
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

    fn tick(&mut self, world: &mut hecs::World, dt: f32, _data: &DataStore) {
        // Passive income (v0.747, REAL): 1 credit per GAME day (1200 s, the
        // TimeSystem day length) into every Wallet — "nobody is ever stuck at
        // zero" (economy.ron's design note). Was a TODO log line since the
        // system was written.
        self.passive_timer += dt;
        let day_seconds = crate::systems::time::SECONDS_PER_DAY as f32;
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
        let receipt = vendor_sell(&mut inv, &mut credits, &goods, "iron_ore_0", 2).unwrap();
        assert!(receipt.contains("4 CR"), "{receipt}");
        assert_eq!(credits, 10);
        assert_eq!(inv.count_item("iron_ore_0"), 0);

        // Selling what you don't have: refused.
        assert!(vendor_sell(&mut inv, &mut credits, &goods, "iron_ore_0", 1).is_err());
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

        // Winter kit: coat (0.6 cold) + beanie (0.1) + winter gloves (0.1).
        let worn = ["coat_winter_0", "hat_beanie_0", "gloves_winter_0"];
        let cold = reg.stat_add_total(worn.iter().copied(), "cold_resist");
        assert!((cold - 0.8).abs() < 1e-4, "kit totals 0.8 cold resist, got {cold}");

        // Hiking boots multiply speed.
        let speed = reg.net_stat_multiplier(["boots_hiking_0"].iter().copied(), "speed");
        assert!((speed - 1.05).abs() < 1e-4, "boots are 1.05x, got {speed}");

        // Backpacks add carry kg.
        let carry = reg.stat_add_total(["backpack_large_0"].iter().copied(), "carry_capacity");
        assert!((carry - 25.0).abs() < 1e-4, "large pack adds 25 kg, got {carry}");
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
