//! HumanityOS Game Systems
//!
//! All gameplay logic — farming, construction, combat, quests, economy, etc.
//! Every system is data-driven: configuration loaded from CSV, RON, or TOML files.

pub mod time;
pub mod farming;
pub mod construction;
pub mod door_anim;
pub mod inventory;
/// Carrying weight: the limit where the player stands (it follows gravity)
/// and what an overload does in each mode (BUG-136, 2026-10-04).
pub mod encumbrance;
/// What the home's machines hold (battery charge, tank litres, vessel
/// contents), kept across a restart and across world entry (2026-09-27).
pub mod machine_levels;
pub mod combat;
pub mod quests;
pub mod crafting;
pub mod vehicles;
pub mod livestock;
pub mod abilities;
pub mod ai;
pub mod skills;
pub mod ecology;
pub mod economy;
pub mod player;
pub mod interaction;
pub mod weather;
pub mod weather_events;
pub mod hydrology;
pub mod atmosphere;
pub mod body_environment;
/// Environment Layer 1: each world's analytic climate (temperature, pressure,
/// prevailing wind), the base field under the weather.
/// docs/design/environment-fields.md.
pub mod env_layer1;
/// What falls, and whether as rain or snow: the air where it falls decides
/// (2026-09-27, Jennings et al. 2018). Every precipitation reader goes here.
pub mod precipitation;
pub mod disasters;
pub mod electrical;
pub mod plumbing;
pub mod solar;
pub mod fire;
pub mod medical;
pub mod status_effects;
pub mod flight;
pub mod food;
/// Body heat: the Gagge two-node heat balance behind the core temperature (2026-09-27).
pub mod body_heat;
/// Sleeping in a bed: the night runs fast and the body wakes rested (2026-09-27).
pub mod sleep;
/// What dying costs: Simplified loses nothing, Realistic leaves the backpack's contents in a
/// pack where the player fell, to go back for (2026-10-04, the operator's decision).
pub mod death_pack;
/// Fluids are litres: tap water for recipes, vessels filled at a tank.
pub mod fluids;
pub mod mining;
pub mod governance;
pub mod docking;
pub mod aging;
pub mod creative_arts;
pub mod geology;
pub mod oceanography;
pub mod astronomy;
pub mod manufacturing;
pub mod waste;
pub mod genetics;
pub mod transportation;
pub mod offline;
pub mod self_sufficiency;
pub mod grow_machines;
/// Ship life support: the home's air and the garden's water as closed loops (2026-09-26).
pub mod life_support;
pub mod ship_power;

/// Push a one-shot SFX request onto the shared `"sfx_events"` DataStore
/// channel (v0.985): ECS systems (construction, crafting) have no engine
/// state, so they emit (catalog id, fallback path) pairs here; the native
/// client's audio frame-sync drains the channel alongside
/// `EngineState::pending_sfx`. Lives HERE (not in the native-gated audio
/// module) so relay builds compile: on a headless relay the channel is
/// simply never registered and this no-ops - the same degradation contract
/// as `quests::push_quest_event`.
pub fn push_sfx_event(
    data: &crate::hot_reload::data_store::DataStore,
    id: &str,
    fallback: &str,
) {
    if let Some(lock) = data.get::<std::sync::Mutex<Vec<(String, String)>>>("sfx_events") {
        if let Ok(mut events) = lock.lock() {
            events.push((id.to_string(), fallback.to_string()));
        }
    }
}
