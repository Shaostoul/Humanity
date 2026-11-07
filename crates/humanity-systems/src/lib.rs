//! humanity-systems: the simulation, what happens in the world and why (the crate split,
//! docs/design/crate-split.md; its card is docs/crates/humanity-systems.md). Farming, building,
//! crafting, food, the body, weather and the environment, creatures, the economy and the quests, each
//! the `System`s the engine runs every tick; the save's shape (`persistence`), the same for the
//! desktop app and a server that holds a shared world; and walking and moving on a planet
//! (`surface_walk`, `surface_move`). Every system is data-driven: its configuration is loaded from
//! CSV, RON or TOML files. It draws nothing, and the server compiles it too.
//!
//! It uses humanity-core, humanity-ship and humanity-terrain. Cut out of the one crate in rung 2
//! (2026-11-02): what was the `systems` module is the crate root (`humanity_systems::farming`), and
//! `persistence`, `surface_walk` and `surface_move` kept their names under it
//! (`humanity_systems::persistence::WorldSave`). Feature flags: `native` (the desktop app's parts;
//! the root's `native` turns it on) and `test-support` (the fixtures other crates' tests use, turned
//! on only by their [dev-dependencies]).

/// Planetary-surface FPS math (task #76 increment 1): tangent-frame camera
/// basis (up = local radial) + stand/walk ground clamp. Pure glam, so its unit
/// tests run under every feature set.
pub mod surface_walk;
/// The player's surface MOVEMENT MODEL (v0.1109): walk vs dev flight, the
/// tangential/radial wish split, and the per-mode vertical integration. Split
/// out of the frame-lock block in the engine's lib.rs so it is testable headless.
pub mod surface_move;
// The save format and where saves live compile into the relay too
// (2026-09-28): a server that holds a shared world needs the same save
// shape the desktop app writes. save_load (applying a save to the local
// world) stays native, in the engine.
pub mod persistence;

pub mod time;
/// The two channels a system speaks to the person on: the world's own lines (held until the person
/// enters a world) and replies to a page action (shown at once). BUG-258, 2026-11-06.
pub mod notices;
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
/// What each home machine drew over its last game day, the record every daily-energy figure
/// reads (2026-10-22).
// The machines' day record is `machines::day_energy` (the crate split's rung 0); its tests, which
// run the ElectricalSystem, stay here.
#[cfg(test)]
mod day_energy_tests;
// Tests of the ship and terrain that reach the simulation, moved up from beside their code (the crate
// split, 2026-10-31): crossing_tests/mod.rs.
#[cfg(test)]
mod crossing_tests;
pub mod combat;
pub mod quests;
pub mod crafting;
pub mod vehicles;
pub mod livestock;
/// Farm animals need care: feed from home storage, water from the tanks, and what going
/// without does in each mode (2026-10-12, first-hour audit S6).
pub mod animal_care;
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
pub mod status_effects;
pub mod flight;
pub mod food;
/// Body heat: the Gagge two-node heat balance behind the core temperature (2026-09-27).
pub mod body_heat;
/// Illness: what a stomach illness from food does to the body's water, over game hours,
/// and what helps (BUG-162, 2026-10-05).
pub mod illness;
/// Medical items: what the Inventory's Use button does with each, from
/// data/medical/treatments.ron (BUG-162, 2026-10-05).
pub mod treatment;
/// Wounds and healing: what a bite or a burn leaves, how it passes, and how a body heals by
/// itself, from data/medical/wounds.ron (first-hour audit S8, 2026-10-12). Applied by the
/// food system, which owns the body; it replaced the never-run MedicalSystem.
pub mod wounds;
/// Falls: the speed a landing meets the floor at, and the injury it makes, from
/// data/medical/falls.ron (2026-10-20). Given to the body as a wound (`wounds::Injury::Fall`).
pub mod falls;
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
/// The mess hall's counter away from a server: a free crew meal one a meal time, and drinking
/// water, counted on the save (ship homes increment 6; the shared world's are the server's).
pub mod ship_meals;

/// DataStore key of the game events the simulation raises for the person's notices
/// ([`push_game_event`]; notifications rung 3).
pub const GAME_EVENTS_KEY: &str = "game_events";

/// Raise a game event for the person's notices (notifications rung 3, docs/design/notifications.md
/// section 11): `id` a row of `data/notifications/notifications.json`'s `game_events`, `place`
/// where it happened (a grow area's name, or empty). Raised at a TRANSITION (a crop's stage
/// reaching ripe, the tanks crossing a tenth), never every tick; the engine folds them, so a farm
/// of 2,000 crops ripening at once is one notice (`notifications::game`). The `sfx_events` contract:
/// the native client registers the list and drains it each frame (`engine/notify.rs`); on a
/// headless relay or in a lib test nothing registers it and this does nothing. Nothing is networked.
pub fn push_game_event(data: &humanity_core::hot_reload::data_store::DataStore, id: &str, place: &str) {
    if let Some(lock) = data.get::<std::sync::Mutex<Vec<(String, String)>>>(GAME_EVENTS_KEY) {
        if let Ok(mut events) = lock.lock() {
            // A bound, in case nothing drains it (a run with no frame): far past any one frame's.
            if events.len() < 10_000 {
                events.push((id.to_string(), place.to_string()));
            }
        }
    }
}

/// Push a one-shot SFX request onto the shared `"sfx_events"` DataStore
/// channel (v0.985): ECS systems (construction, crafting) have no engine
/// state, so they emit (catalog id, fallback path) pairs here; the native
/// client's audio frame-sync drains the channel alongside
/// `EngineState::pending_sfx`. Lives HERE (not in the native-gated audio
/// module) so relay builds compile: on a headless relay the channel is
/// simply never registered and this no-ops - the same degradation contract
/// as `quests::push_quest_event`.
pub fn push_sfx_event(
    data: &humanity_core::hot_reload::data_store::DataStore,
    id: &str,
    fallback: &str,
) {
    if let Some(lock) = data.get::<std::sync::Mutex<Vec<(String, String)>>>("sfx_events") {
        if let Ok(mut events) = lock.lock() {
            events.push((id.to_string(), fallback.to_string()));
        }
    }
}
