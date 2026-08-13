//! HumanityOS Game Systems
//!
//! All gameplay logic — farming, construction, combat, quests, economy, etc.
//! Every system is data-driven: configuration loaded from CSV, RON, or TOML files.

pub mod time;
pub mod farming;
pub mod construction;
pub mod door_anim;
pub mod inventory;
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
pub mod disasters;
pub mod electrical;
pub mod plumbing;
pub mod solar;
pub mod hvac;
pub mod fire;
pub mod medical;
pub mod status_effects;
pub mod food;
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
