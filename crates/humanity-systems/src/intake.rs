//! The intake ledger (2026-11-20, engine piece P3 of docs/design/quest-chains.md, sections 2.10
//! and 4.2): what the body took in over the last game day, hour by hour.
//!
//! The vitals say how full a person is NOW (Hydration, Satiation); nothing said what they took in
//! to get there. "Take in 2.5 L of water in one day" and "Eat 2,000 kcal of food in one day" need
//! that number, and it is not a copy of anything the body systems keep: Hydration is capped at
//! its maximum and drains all day, so a full meter after a litre and after three litres reads
//! the same. The ledger is the body's own record of what went in, written where it goes in (the
//! food system's `consume`, for every food and drink), never by scanning.
//!
//! - 24 hourly buckets on the game clock (an hour is always 3,600 game seconds, CLAUDE.md's one
//!   game clock), newest last; a bucket older than the last 24 game hours is dropped when the
//!   next one is written. "The last day" is the rolling 24 game hours up to now (`last_day`).
//! - Each bucket carries what the food data carries per food (data/food_system.ron): water,
//!   food energy, protein, carbohydrate, fat, fibre, the vitamins as shares of the US FDA Daily
//!   Value and the minerals in milligrams, for the amount actually eaten (`portion_g`), times
//!   what the body kept (spoiled food feeds a quarter; an illness that takes water keeps only
//!   part of a drink's water, `Illnesses::drink_kept`).
//! - Saved with the body (`persistence::BodySave::intake`), so a save and load keeps the day's
//!   record; the time the game was closed passes no game time, so it neither counts nor ages it.
//!
//! The quests read it as the `intake.*` readings (`quests::readings`).

use serde::{Deserialize, Serialize};

/// The seconds in a game hour (the one game clock: an hour never changes length).
const HOUR_S: f64 = 3600.0;
/// How many game hours "the last day" spans.
pub const WINDOW_H: i64 = 24;

/// What went in: one meal or drink, an hour of them, or a day.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Intake {
    /// Litres of water (a kilogram a litre), drinks and food's own water together.
    pub water_l: f64,
    /// Food energy, kilocalories.
    pub kcal: f64,
    pub protein_g: f64,
    pub carbs_g: f64,
    pub fat_g: f64,
    pub fiber_g: f64,
    /// Vitamins A, B (the six B vitamins' mean, data/food_system.ron's header), C, D, E and K,
    /// each as a share of its US FDA Daily Value (1.0 = one day's value), as the food data has them.
    pub vitamins_dv: [f64; 6],
    /// Iron, calcium, zinc and potassium, milligrams.
    pub minerals_mg: [f64; 4],
}

impl Intake {
    /// This plus `other`.
    pub fn plus(&self, other: &Intake) -> Intake {
        let mut v = self.vitamins_dv;
        let mut m = self.minerals_mg;
        for (a, b) in v.iter_mut().zip(other.vitamins_dv) {
            *a += b;
        }
        for (a, b) in m.iter_mut().zip(other.minerals_mg) {
            *a += b;
        }
        Intake {
            water_l: self.water_l + other.water_l,
            kcal: self.kcal + other.kcal,
            protein_g: self.protein_g + other.protein_g,
            carbs_g: self.carbs_g + other.carbs_g,
            fat_g: self.fat_g + other.fat_g,
            fiber_g: self.fiber_g + other.fiber_g,
            vitamins_dv: v,
            minerals_mg: m,
        }
    }
}

/// One game hour of the ledger.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct IntakeHour {
    /// The game hour it is: whole game hours since the clock's start (elapsed seconds / 3,600).
    pub hour: i64,
    pub took: Intake,
}

/// The ledger: an ECS component on the player, saved with the body.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct IntakeLedger {
    /// The last day's hours that had any intake, oldest first.
    pub hours: Vec<IntakeHour>,
}

/// The game hour `elapsed_s` falls in.
pub fn hour_of(elapsed_s: f64) -> i64 {
    (elapsed_s.max(0.0) / HOUR_S).floor() as i64
}

impl IntakeLedger {
    /// Write what went in at game time `elapsed_s` (game seconds): into its hour's bucket, and
    /// the buckets older than the last day dropped. A clock set back (a rig's time verb) starts
    /// the record again from that hour rather than counting the future twice.
    pub fn add(&mut self, elapsed_s: f64, took: &Intake) {
        let h = hour_of(elapsed_s);
        self.hours.retain(|b| b.hour <= h && b.hour > h - WINDOW_H);
        match self.hours.last_mut() {
            Some(last) if last.hour == h => last.took = last.took.plus(took),
            _ => self.hours.push(IntakeHour { hour: h, took: *took }),
        }
    }

    /// What went in over the last 24 game hours up to `elapsed_s` (the current hour and the 23
    /// before it).
    pub fn last_day(&self, elapsed_s: f64) -> Intake {
        let h = hour_of(elapsed_s);
        self.hours
            .iter()
            .filter(|b| b.hour <= h && b.hour > h - WINDOW_H)
            .fold(Intake::default(), |sum, b| sum.plus(&b.took))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn water(l: f64) -> Intake {
        Intake { water_l: l, ..Intake::default() }
    }

    /// A drink counts for 24 game hours and then drops out; drinks in one hour share a bucket.
    #[test]
    fn a_drink_counts_for_one_rolling_day() {
        let mut ledger = IntakeLedger::default();
        ledger.add(10.0 * HOUR_S + 5.0, &water(0.5));
        ledger.add(10.0 * HOUR_S + 900.0, &water(0.25));
        assert_eq!(ledger.hours.len(), 1, "two drinks in one hour share its bucket");
        ledger.add(20.0 * HOUR_S, &water(1.0));
        assert!((ledger.last_day(20.0 * HOUR_S).water_l - 1.75).abs() < 1e-9);
        // 33:59 still has hour 10 (hours 10 to 33); at 34:00 it is gone.
        assert!((ledger.last_day(33.0 * HOUR_S + 3599.0).water_l - 1.75).abs() < 1e-9);
        assert!((ledger.last_day(34.0 * HOUR_S).water_l - 1.0).abs() < 1e-9);
        ledger.add(50.0 * HOUR_S, &water(0.1));
        assert_eq!(ledger.hours.len(), 1, "writing drops the hours older than a day");
    }
}
