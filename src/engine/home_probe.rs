//! The home's air, water and power as the HUD reads them, for the dev IPC's recorder (the
//! `home_systems` block of `debug/remote_players_done.json`, engine/ipc.rs) and the dev MCP's
//! `game_home_state` tool (engine/mcp). One reader for both, so a rig and an AI session see the
//! same numbers the player sees: the GuiState mirrors lib.rs fills each frame from the
//! DataStore's `power_status`, `water_status` and `air_status` (systems/electrical.rs,
//! systems/plumbing.rs, systems/atmosphere.rs). Permanent dev tooling (2026-10-22).

use crate::gui::GuiState;

/// The home's power, water and air right now, as the HUD and the Home card show them.
pub(crate) fn json(gs: &GuiState) -> serde_json::Value {
    let ship = &gs.power_ship;
    serde_json::json!({
        "power": {
            "generation_w": gs.power_generation,
            "consumption_w": gs.power_consumption,
            "balance_w": gs.power_balance,
            "battery_wh": gs.power_battery_wh,
            "battery_capacity_wh": gs.power_battery_capacity_wh,
            "autonomy_hours": gs.power_autonomy_hours,
            // The ship's reactor feed (systems/ship_power.rs): whether the home is tied to the
            // ship's bus, what it draws and gives back now, and the metered totals.
            "ship_fed": ship.fed,
            "ship_drawn_w": ship.drawn_w,
            "ship_returned_w": ship.returned_w,
            "ship_drawn_wh": ship.drawn_wh,
            "ship_returned_wh": ship.returned_wh,
        },
        "water": {
            "production_lpm": gs.water_production_lpm,
            "demand_lpm": gs.water_demand_lpm,
            "stored_l": gs.water_stored_l,
            "capacity_l": gs.water_capacity_l,
            "days_autonomy": gs.water_days_autonomy,
        },
        "air": {
            "o2_pct": gs.air_o2_pct,
            "co2_pct": gs.air_co2_pct,
            "pressure_atm": gs.air_pressure_atm,
            "temp_c": gs.air_temp_c,
            "breathable": gs.air_breathable,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every number comes from its own mirror, under the name the tools read it by: a swapped or
    /// dropped field would hand a rig or an AI session a wrong reading that looks like a right one.
    #[test]
    fn each_reading_comes_from_its_own_mirror() {
        let mut gs = GuiState::default();
        gs.power_generation = 1.0;
        gs.power_consumption = 2.0;
        gs.power_balance = 3.0;
        gs.power_battery_wh = 4.0;
        gs.power_battery_capacity_wh = 5.0;
        gs.power_autonomy_hours = 6.0;
        gs.power_ship.fed = true;
        gs.power_ship.drawn_w = 7.0;
        gs.power_ship.returned_w = 8.0;
        gs.water_production_lpm = 9.0;
        gs.water_demand_lpm = 10.0;
        gs.water_stored_l = 11.0;
        gs.water_capacity_l = 12.0;
        gs.water_days_autonomy = 13.0;
        gs.air_o2_pct = 14.0;
        gs.air_co2_pct = 15.0;
        gs.air_pressure_atm = 16.0;
        gs.air_temp_c = 17.0;
        gs.air_breathable = true;
        let j = json(&gs);
        let n = |p: &str| j.pointer(p).and_then(|v| v.as_f64()).unwrap_or(-1.0);
        assert_eq!(n("/power/generation_w"), 1.0);
        assert_eq!(n("/power/consumption_w"), 2.0);
        assert_eq!(n("/power/balance_w"), 3.0);
        assert_eq!(n("/power/battery_wh"), 4.0);
        assert_eq!(n("/power/battery_capacity_wh"), 5.0);
        assert_eq!(n("/power/autonomy_hours"), 6.0);
        assert_eq!(j.pointer("/power/ship_fed"), Some(&serde_json::json!(true)));
        assert_eq!(n("/power/ship_drawn_w"), 7.0);
        assert_eq!(n("/power/ship_returned_w"), 8.0);
        assert_eq!(n("/water/production_lpm"), 9.0);
        assert_eq!(n("/water/demand_lpm"), 10.0);
        assert_eq!(n("/water/stored_l"), 11.0);
        assert_eq!(n("/water/capacity_l"), 12.0);
        assert_eq!(n("/water/days_autonomy"), 13.0);
        assert_eq!(n("/air/o2_pct"), 14.0);
        assert_eq!(n("/air/co2_pct"), 15.0);
        assert_eq!(n("/air/pressure_atm"), 16.0);
        assert_eq!(n("/air/temp_c"), 17.0);
        assert_eq!(j.pointer("/air/breathable"), Some(&serde_json::json!(true)));
    }
}
