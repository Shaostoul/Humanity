//! A person's own presence setting (`privacy_update`): hide or show themselves as online on this
//! server (2026-08-23; storage/members.rs `set_hide_presence`).
//!
//! The apps send the setting again on every fresh sign-in (src/engine/frame_ws_poll.rs, so a new
//! or wiped server learns the person's choice at once), and this server used to answer every one
//! with a confirmation line to every socket the key holds. With a desktop and a phone signed in as
//! the same person, each reconnect of either added another "Presence visible: ..." line to both
//! (the operator, 2026-11-22: "This text keeps reappearing ... It got up to 3 repeats"). Now the
//! line and the roster refresh happen only when the stored setting actually changes; a repeat of
//! the same choice is stored silently.

use std::sync::Arc;

use crate::relay::relay::{RelayMessage, RelayState};

/// What a person is told when their presence setting changes.
pub const PRESENCE_HIDDEN_WORDS: &str =
    "Presence hidden: you no longer appear online, no last-seen is stored, and no join/leave or typing signals are sent.";
/// What a person is told when they choose to appear online.
pub const PRESENCE_VISIBLE_WORDS: &str = "Presence visible: you appear online to other members.";

/// Store `hide` for `key`; when that changed what was stored, refresh everyone's roster and tell
/// the person in words. A repeat of the stored choice changes nothing anyone sees.
pub async fn handle_privacy_update(state: &Arc<RelayState>, key: &str, hide: bool) {
    let was_hidden = state.db.presence_hidden(key);
    if let Err(e) = state.db.set_hide_presence(key, hide) {
        tracing::error!("privacy_update failed: {e}");
        return;
    }
    if was_hidden == hide {
        return;
    }
    tracing::info!(
        "privacy_update: presence {} for {}…",
        if hide { "hidden" } else { "visible" },
        humanity_core::text::prefix(key, 12)
    );
    // Roster refresh so the change is immediate for everyone.
    crate::relay::handlers::broadcast::broadcast_full_user_list(state).await;
    let _ = state.broadcast_tx.send(RelayMessage::Private {
        to: key.to_string(),
        message: if hide { PRESENCE_HIDDEN_WORDS } else { PRESENCE_VISIBLE_WORDS }.to_string(),
    });
}

#[cfg(test)]
#[path = "presence_tests.rs"]
mod presence_tests;
