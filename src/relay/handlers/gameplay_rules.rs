//! A server's gameplay rules reaching the games in its shared world (2026-11-21;
//! storage/gameplay_rules.rs holds the ruleset, docs/design/server-rules.md says why).
//!
//! A game is sent the ruleset in its `game_welcome` (msg_handlers.rs, [`welcome_field`]), and
//! when an admin changes it (`server_settings_update`, server_settings_update.rs) every socket is
//! told at once with a `game_rules`, the way the world clock's speed reaches them: a game counts
//! it only while it is in the shared world (engine/server_rules.rs), so a chat-only client or a
//! solo game ignores it. No restart, and nothing sent when the rules did not change.

use crate::relay::relay::{RelayMessage, RelayState};
use crate::relay::storage::GameplayRules;
use std::sync::Arc;

/// The welcome's `rules`: the server's ruleset as a game is sent it.
pub fn welcome_field(state: &RelayState) -> serde_json::Value {
    state.db.get_server_settings().unwrap_or_default().gameplay_rules.wire()
}

/// The `game_rules` message every game is sent when the rules change.
pub fn rules_message(rules: &GameplayRules) -> serde_json::Value {
    serde_json::json!({ "type": "game_rules", "rules": rules.wire() })
}

/// After an admin's update was saved: tell every connected game the new rules, when they
/// changed (`before` is the ruleset the update started from). Nothing on a server with the game
/// switched off.
pub fn announce(state: &Arc<RelayState>, before: &GameplayRules, now: &GameplayRules) {
    if before == now || !state.features.enabled(crate::relay::features::Feature::Game) {
        return;
    }
    tracing::info!("The shared world's gameplay rules are now {}", now.preset);
    let _ = state.broadcast_tx.send(RelayMessage::System { message: format!("__game__:{}", rules_message(now)) });
}

#[cfg(test)]
mod tests {
    use crate::relay::relay::{RelayMessage, RelayState};
    use crate::relay::storage::Storage;

    fn frame(v: serde_json::Value) -> RelayMessage {
        serde_json::from_value::<RelayMessage>(v).expect("the frame parses")
    }

    /// The `game_rules` messages the relay put on its broadcast since `rx` subscribed.
    fn rules_sent(rx: &mut tokio::sync::broadcast::Receiver<RelayMessage>) -> Vec<serde_json::Value> {
        let mut out = Vec::new();
        while let Ok(m) = rx.try_recv() {
            if let RelayMessage::System { message } = m {
                if let Some(v) = message.strip_prefix("__game__:").and_then(|p| serde_json::from_str::<serde_json::Value>(p).ok()) {
                    if v["type"] == "game_rules" {
                        out.push(v);
                    }
                }
            }
        }
        out
    }

    /// A MEMBER WHO IS NOT AN ADMIN CANNOT CHANGE THE RULES (2026-11-21): a member's
    /// `server_settings_update` naming Realistic leaves the server Forgiving and tells no game
    /// anything, and so does a moderator's (a moderator is not an admin).
    ///
    /// Seen red 2026-11-21 with the update handler's role check switched off: "member, who is not
    /// an admin, changed the rules / left: \"realistic\" / right: \"forgiving\"".
    #[tokio::test]
    async fn a_member_who_is_not_an_admin_cannot_change_the_rules() {
        let state = std::sync::Arc::new(RelayState::new(Storage::open_temp("gameplay_rules_member")));
        state.db.set_role("moddy", "mod").unwrap();
        let mut rx = state.broadcast_tx.subscribe();
        let realistic = serde_json::json!({ "type": "server_settings_update", "gameplay_rules": { "preset": "realistic" } });
        for who in ["member", "moddy"] {
            super::super::server_settings_update::handle(&state, who, frame(realistic.clone())).await;
            assert_eq!(state.db.get_server_settings().unwrap().gameplay_rules.preset, "forgiving", "{who}, who is not an admin, changed the rules");
        }
        assert!(rules_sent(&mut rx).is_empty(), "a refused change tells no game anything");
    }

    /// AN ADMIN'S CHANGE REACHES THE GAMES (2026-11-21): the admin's `server_settings_update`
    /// saves Realistic and puts one `game_rules` with Realistic's levels on the broadcast every
    /// socket hears; the same rules again send nothing; the welcome's field is the saved ruleset;
    /// and a preset the file does not list changes nothing.
    ///
    /// Seen red 2026-11-21 with `announce` taken out of the update handler: "the admin's change
    /// told the games / left: 0 / right: 1".
    #[tokio::test]
    async fn an_admins_change_reaches_the_games() {
        let state = std::sync::Arc::new(RelayState::new(Storage::open_temp("gameplay_rules_update")));
        state.db.set_role("op", "admin").unwrap();
        let mut rx = state.broadcast_tx.subscribe();
        let realistic = serde_json::json!({ "type": "server_settings_update", "gameplay_rules": { "preset": "realistic" } });
        super::super::server_settings_update::handle(&state, "op", frame(realistic.clone())).await;
        assert_eq!(state.db.get_server_settings().unwrap().gameplay_rules.preset, "realistic", "the admin's change is saved");
        let sent = rules_sent(&mut rx);
        assert_eq!(sent.len(), 1, "the admin's change told the games");
        assert_eq!(sent[0]["rules"]["preset"], "realistic");
        assert_eq!(sent[0]["rules"]["levels"]["body_heat"], "realistic", "with the levels in play, not only the preset's name");
        super::super::server_settings_update::handle(&state, "op", frame(realistic)).await;
        assert!(rules_sent(&mut rx).is_empty(), "the same rules again: nothing to tell");
        assert_eq!(super::welcome_field(&state)["preset"], "realistic", "a game joining now is welcomed with them");
        // A preset the file does not list changes nothing.
        super::super::server_settings_update::handle(&state, "op", frame(serde_json::json!({ "type": "server_settings_update", "gameplay_rules": { "preset": "nightmare" } }))).await;
        assert_eq!(state.db.get_server_settings().unwrap().gameplay_rules.preset, "realistic", "not a preset: nothing changed");
    }
}
