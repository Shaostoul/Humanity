//! The readings a `Keep` step can name (2026-11-20, E2 of docs/design/quest-chains.md 2.10):
//! a number the body or its intake ledger holds now, read off the player's own components.
//!
//! The list is code, not data, because every reading is one match arm reading one field: a
//! reading in a quest file that is not here could never be met, so the shipped-quest test refuses
//! one (`shipped_quest_objective_ids_all_resolve`). The words the Quests page shows for each come
//! from data/i18n/en.json by key (`label_key`). The design named a data file of labels
//! (data/quests/readings.ron); words live in en.json, so the list is only this table.
//!
//! - `vital.*`: the `Vitals` component, 0 to its maximum (Hydration, Satiation, Energy, Oxygen,
//!   Waste) and the core temperature in Celsius.
//! - `intake.*`: the intake ledger's last 24 game hours (`intake.rs`, P3).

use humanity_core::ecs::components::Vitals;

use crate::intake::IntakeLedger;

/// Every reading a Keep step may name, and its unit's key in en.json (`quests.unit_*`).
pub const READINGS: &[(&str, &str)] = &[
    ("vital.hydration", "quests.unit_points"),
    ("vital.satiation", "quests.unit_points"),
    ("vital.energy", "quests.unit_points"),
    ("vital.oxygen", "quests.unit_points"),
    ("vital.waste", "quests.unit_points"),
    ("vital.body_temp_c", "quests.unit_celsius"),
    ("intake.water_l_24h", "quests.unit_litres"),
    ("intake.kcal_24h", "quests.unit_kcal"),
    ("intake.protein_g_24h", "quests.unit_grams"),
];

/// Whether `id` is a reading.
pub fn known(id: &str) -> bool {
    READINGS.iter().any(|(r, _)| *r == id)
}

/// The en.json key of a reading's name: "vital.hydration" -> "quests.reading_vital_hydration".
pub fn label_key(id: &str) -> String {
    format!("quests.reading_{}", id.replace('.', "_"))
}

/// The en.json key of a reading's unit.
pub fn unit_key(id: &str) -> Option<&'static str> {
    READINGS.iter().find(|(r, _)| *r == id).map(|(_, u)| *u)
}

/// What a reading is read from: the player's body and its ledger, at game time `now_s`.
#[derive(Clone, Copy)]
pub struct Body<'a> {
    pub vitals: Option<&'a Vitals>,
    pub intake: Option<&'a IntakeLedger>,
    /// The game clock, seconds (`time::elapsed_now`).
    pub now_s: f64,
}

/// The reading's value now; None when it is not a reading or the body lacks what it reads.
pub fn read(id: &str, body: &Body) -> Option<f32> {
    let v = body.vitals;
    let day = || body.intake.map(|l| l.last_day(body.now_s));
    match id {
        "vital.hydration" => v.map(|v| v.hydration),
        "vital.satiation" => v.map(|v| v.satiation),
        "vital.energy" => v.map(|v| v.energy),
        "vital.oxygen" => v.map(|v| v.oxygen),
        "vital.waste" => v.map(|v| v.waste),
        "vital.body_temp_c" => v.map(|v| v.body_temp_c),
        "intake.water_l_24h" => day().map(|d| d.water_l as f32),
        "intake.kcal_24h" => day().map(|d| d.kcal as f32),
        "intake.protein_g_24h" => day().map(|d| d.protein_g as f32),
        _ => None,
    }
}

/// Whether `value` is in the band `min`..=`max` (either open).
pub fn in_band(value: f32, min: Option<f32>, max: Option<f32>) -> bool {
    min.is_none_or(|m| value >= m) && max.is_none_or(|m| value <= m)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every reading in the table reads from a body that has what it names; the table and the
    /// match never drift apart.
    #[test]
    fn every_reading_in_the_table_reads() {
        let vitals = Vitals::default();
        let ledger = IntakeLedger::default();
        let body = Body { vitals: Some(&vitals), intake: Some(&ledger), now_s: 0.0 };
        for (id, _) in READINGS {
            assert!(read(id, &body).is_some(), "{id} is in the table but reads nothing");
        }
        assert!(read("vital.mood", &body).is_none() && !known("vital.mood"));
        assert!(in_band(60.0, Some(60.0), None) && !in_band(59.9, Some(60.0), None) && in_band(38.0, None, Some(38.0)));
    }
}
