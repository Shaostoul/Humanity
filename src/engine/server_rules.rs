//! A server's gameplay rules into the game (2026-11-21; docs/design/server-rules.md). The relay
//! sends its ruleset in the `game_welcome` (handlers/gameplay_rules.rs) and a `game_rules` to
//! every socket when an admin changes it; this keeps it in `SettingsState::server_rules` while
//! the game is in that server's shared world, where every system reads its levels
//! (gui/server_rules.rs), and drops it when the game leaves the world (home_plot.rs
//! `forget_shared_world`), so the person's own levels play again. Pure on the GUI's state, so it
//! is tested without an engine; `engine::net_route` routes the messages here.

use crate::gui::GuiState;
use crate::relay::storage::GameplayRules;

/// A welcome: the server's rules from now on (none from a relay that sends none: then the
/// person's own levels play).
pub(crate) fn on_welcome(gui: &mut GuiState, v: &serde_json::Value) {
    let rules = v.get("rules").and_then(GameplayRules::from_wire);
    if let Some(r) = &rules {
        log::info!("Gameplay: this server's rules, {}", r.preset);
    }
    gui.settings.server_rules = rules;
}

/// A `game_rules` (an admin changed them): taken while in the shared world, ignored otherwise
/// (every socket hears it, chat-only and solo ones too). True when the message was one.
pub(crate) fn on_game_message(gui: &mut GuiState, v: &serde_json::Value, in_shared_world: bool) -> bool {
    if v.get("type").and_then(|t| t.as_str()) != Some("game_rules") {
        return false;
    }
    if in_shared_world {
        if let Some(r) = v.get("rules").and_then(GameplayRules::from_wire) {
            log::info!("Gameplay: this server's rules are now {}", r.preset);
            gui.settings.server_rules = Some(r);
        }
    }
    true
}

/// Out of the shared world: the person's own levels again.
pub(crate) fn forget(gui: &mut GuiState) {
    gui.settings.server_rules = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use humanity_core::hot_reload::data_store::DataStore;
    use humanity_systems::{body_heat, death_pack, mining};

    /// What the systems are told, as the frame's bridges publish it.
    fn published(gui: &mut GuiState) -> (body_heat::Mode, death_pack::DeathMode, mining::Mode, f32) {
        let mut data = DataStore::new();
        data.insert(mining::MODE_KEY, std::sync::Mutex::new(mining::Mode::Realistic)); // the slot sim_boot registers
        crate::engine::survival_env::publish_levels(gui, &mut data);
        crate::engine::drone::bridge_gui(gui, &data);
        let heat = *data.get::<body_heat::Mode>(body_heat::MODE_KEY).expect("body heat published");
        let death = *data.get::<death_pack::DeathMode>(death_pack::MODE_KEY).expect("death published");
        (heat, death, mining::Mode::from_store(&data), gui.settings.vitals_drain_rate())
    }

    /// A JOINED GAME'S SYSTEMS PLAY THE SERVER'S LEVELS, A CHANGE REACHES THEM, AND LEAVING GIVES
    /// THE PERSON'S BACK (2026-11-21). A person who chose Off for body heat, death and the drone,
    /// and a paused drain, is welcomed into a Forgiving server: the systems are told Forgiving's
    /// levels (each setting's default), not Off. The admin switches to Realistic: the `game_rules`
    /// every socket hears reaches them. Out of the world the same message is ignored, and the
    /// person's own Off plays again.
    ///
    /// Seen red 2026-11-21 with `SettingsState::level` reading the person's own map as before the
    /// rules (so the bridges published the person's levels while joined): "a Forgiving server's
    /// body heat / left: Off / right: Forgiving".
    #[test]
    fn a_joined_games_systems_play_the_servers_levels_and_leaving_gives_the_persons_back() {
        let mut gui = GuiState::default();
        for id in ["body_heat", "death", "mining_drone"] {
            gui.settings.set_level(id, "off");
        }
        gui.settings.vitals_drain = 0.0;
        let own = published(&mut gui);
        assert_eq!((own.0, own.1, own.2, own.3), (body_heat::Mode::Off, death_pack::DeathMode::Off, mining::Mode::Off, 0.0), "solo: the person's own");

        let forgiving = GameplayRules::default();
        on_welcome(&mut gui, &serde_json::json!({ "type": "game_welcome", "player_id": 7, "rules": forgiving.wire() }));
        let joined = published(&mut gui);
        assert_eq!(joined.0, body_heat::Mode::Forgiving, "a Forgiving server's body heat");
        assert_eq!(joined.1, death_pack::DeathMode::Simplified, "a Forgiving server's death");
        assert_eq!(joined.2, mining::Mode::Simplified, "a Forgiving server's drone");
        assert_eq!(joined.3, 1.0, "a Forgiving server's drain: real time, not the person's pause");

        let realistic = GameplayRules::preset("realistic").unwrap();
        let change = crate::relay::handlers::gameplay_rules::rules_message(&realistic);
        assert!(on_game_message(&mut gui, &change, true), "a game_rules is this module's");
        let changed = published(&mut gui);
        assert_eq!((changed.0, changed.1, changed.2), (body_heat::Mode::Realistic, death_pack::DeathMode::Realistic, mining::Mode::Realistic), "the admin's change reached the systems");

        forget(&mut gui);
        assert!(on_game_message(&mut gui, &change, false), "heard out of the world");
        let left = published(&mut gui);
        assert_eq!((left.0, left.1, left.2, left.3), (body_heat::Mode::Off, death_pack::DeathMode::Off, mining::Mode::Off, 0.0), "left the world: the person's own again, and a change heard outside is ignored");
        assert_eq!(gui.settings.gameplay.get("body_heat").map(String::as_str), Some("off"), "the person's choice was never touched");
    }

    /// THE ROUTER AND THE LEAVING USE THIS MODULE: net_route hands the welcome and a `game_rules`
    /// here, and home_plot's `forget_shared_world` drops the rules (read from the source, as the
    /// shared-build wiring test does: an EngineState is too heavy to build in a lib test).
    ///
    /// Seen red 2026-11-21 with the welcome arm's `server_rules::on_welcome` line taken out of
    /// net_route.rs: "the welcome arm never tells engine::server_rules".
    #[test]
    fn the_router_and_the_leaving_use_this_module() {
        let root = std::path::Path::new(env!("HUMANITY_REPO_ROOT"));
        let route = std::fs::read_to_string(root.join("src/engine/net_route.rs")).unwrap();
        assert!(route.contains("crate::engine::server_rules::on_welcome(&mut state.gui_state, &v)"), "the welcome arm never tells engine::server_rules");
        assert!(route.contains("crate::engine::server_rules::on_game_message(&mut state.gui_state, &v,"), "game_rules never reaches engine::server_rules");
        let plot = std::fs::read_to_string(root.join("src/engine/home_plot.rs")).unwrap();
        let forget = plot.find("pub(crate) fn forget_shared_world(").expect("forget_shared_world");
        let body = &plot[forget..forget + plot[forget..].find("\n}").unwrap()];
        assert!(body.contains("crate::engine::server_rules::forget(&mut state.gui_state)"), "leaving the world keeps the server's rules");
    }
}
