//! Economy system — credits, trading, age-based starting balance.
//!
//! Core mechanic: every player starts with 1 credit per day they've been alive.
//! A 36-year-old starts with ~13,149 credits. A teenager with ~5,475.
//! Passive income: 1 credit per real-time day just for existing.
//!
//! Data: data/economy.ron (formula, earning rates, trade fees)
//!       data/trade_goods.ron (185 item base values)

pub mod fleet;

use crate::hot_reload::data_store::DataStore;
use crate::ecs::systems::System;

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

    fn tick(&mut self, _world: &mut hecs::World, dt: f32, _data: &DataStore) {
        // Passive income: accumulate time, pay out 1 credit per real-time day
        self.passive_timer += dt;
        let day_seconds = 86400.0_f32;
        if self.passive_timer >= day_seconds {
            self.passive_timer -= day_seconds;
            // TODO: add passive_income_per_day credits to player's wallet
            log::debug!("Passive income: +{} credits", self.passive_income_per_day);
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
}
