//! Friend codes on the desktop app: making one, and what the server answers (the 2026-10-10
//! batch review of docs/design/blocking-and-safe-mode.md 10h).
//!
//! A friend code is a short code the server makes for us; whoever redeems it gets our identity
//! back in `friend_code_result.owner_key`, and the RELAY creates no follow (it keeps no social
//! graph since 2026-08-24): the redeemer's own app must follow the owner, which starts the
//! friendship over sealed control messages. The web chat did that; the desktop app had no
//! handler for `friend_code_result` at all, so a code redeemed here followed nobody, and it had
//! none for `friend_code_response` either, so a code made here was never shown.
//!
//! THE PROTECTED SETUP: making a code needs the PIN (`ProtectedAction::MakeFriendCode`), as
//! redeeming one already did, because the code lets someone else start a friendship with this
//! device without asking. The follow a redeemed code leads to goes through Follow's own gate
//! (engine/dm.rs `set_follow`): when the code was redeemed with the PIN, that PIN covers the
//! friend the answer names (`ProtectedUi::code_redeemed`), so it is not asked twice; otherwise
//! (the setup turned on since, say) the PIN prompt opens for the follow like any other.
//!
//! The message pump reaches this file through the catch-all arm of engine/frame_ws_poll.rs,
//! which sits at its file-size budget.

use crate::gui::GuiState;
use crate::net::protected::ProtectedAction;

/// Ask the server for a friend code (Server Settings' "Generate friend code", and a typed
/// `/friend-code` once its PIN is entered). With the protected setup on it needs the PIN: the
/// prompt opens and nothing is sent. True when the request went out.
pub(crate) fn request_code(gs: &mut GuiState) -> bool {
    if !crate::engine::protected::allows(gs, ProtectedAction::MakeFriendCode) {
        return false;
    }
    if !gs.ws_client.as_ref().is_some_and(|c| c.is_connected()) {
        return false;
    }
    crate::gui::pages::chat::send_slash_command(gs, "/friend-code");
    true
}

/// The words shown for a code the server made.
pub(crate) fn code_line(code: &str) -> String {
    format!("Your friend code: {code}. It works once, for 24 hours. Whoever redeems it (Server Settings, or typing /redeem {code}) follows you, and you are friends once you follow them back.")
}

/// The server's friend-code frames: `friend_code_response` (a code we asked for) and
/// `friend_code_result` (the answer to our redeem). True when the frame was one of them.
pub(crate) fn on_frame(gs: &mut GuiState, frame: &serde_json::Value) -> bool {
    match frame.get("type").and_then(|t| t.as_str()) {
        Some("friend_code_response") => {
            let code = frame.get("code").and_then(|c| c.as_str()).unwrap_or("").trim().to_string();
            if !code.is_empty() {
                let line = code_line(&code);
                gs.server_settings_status = line.clone();
                system_line(gs, &line);
            }
            true
        }
        Some("friend_code_result") => {
            on_result(gs, frame);
            true
        }
        _ => false,
    }
}

/// `friend_code_result`: on success, follow the code's owner (which is what makes the friendship
/// start: the relay no longer does it), through Follow's own gate; on failure, say why.
fn on_result(gs: &mut GuiState, frame: &serde_json::Value) {
    // The PIN given for a redeem covers only its own answer.
    let redeemed_with_pin = std::mem::take(&mut gs.protected.code_redeemed);
    let success = frame.get("success").and_then(|s| s.as_bool()) == Some(true);
    if !success {
        let why = frame.get("message").and_then(|m| m.as_str()).unwrap_or("").trim();
        let why = if why.is_empty() { "the server did not say why" } else { why };
        system_line(gs, &format!("The friend code did not work: {why}"));
        return;
    }
    let owner = frame.get("owner_key").and_then(|k| k.as_str()).unwrap_or("").trim().to_string();
    if owner.is_empty() || owner.eq_ignore_ascii_case(&gs.profile_public_key) {
        return;
    }
    let name = frame.get("name").and_then(|n| n.as_str()).filter(|n| !n.trim().is_empty()).map(str::to_string).unwrap_or_else(|| crate::engine::dm::dm_display_name(gs, &owner));
    if redeemed_with_pin && crate::engine::protected::is_on(gs) {
        // The PIN entered for this code: Follow's gate uses it up and approves them.
        gs.protected.granted = Some(ProtectedAction::Follow(owner.clone()));
    }
    crate::engine::dm::set_follow(gs, &owner, true);
    // A permission the follow did not use (it could not start) does not linger.
    if gs.protected.granted == Some(ProtectedAction::Follow(owner.clone())) {
        gs.protected.granted = None;
    }
    let following = gs.dm_store.as_ref().is_some_and(|s| s.is_following(&owner));
    if following {
        system_line(gs, &format!("Friend code accepted: you now follow {name}. You are friends once they follow you back."));
    }
}

/// One line from the app in the chat, where the server's own private notices appear (never
/// into an open direct message or group, where it would vanish on reload).
fn system_line(gs: &mut GuiState, text: &str) {
    let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    gs.chat_messages.push(crate::gui::ChatMessage {
        sender_name: "System".to_string(),
        content: text.to_string(),
        timestamp: crate::gui::pages::chat::format_timestamp(now_ms),
        timestamp_ms: now_ms,
        channel: crate::gui::pages::chat::notice_channel(&gs.chat_active_channel),
        server: crate::gui::pages::chat::norm_server_url(&gs.server_url),
        ..Default::default()
    });
}

#[cfg(test)]
#[path = "friend_code_tests.rs"]
mod tests;
