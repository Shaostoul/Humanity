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

/// How long the server has to answer a friend code asked for or redeemed: an answer later than
/// this, or one this device never asked for, is dropped and logged (the owner branch's seventh
/// review, H1 (c): a server could push `friend_code_result` unasked and the app followed whoever it
/// named).
const ANSWER_WINDOW: std::time::Duration = std::time::Duration::from_secs(120);

/// A command leaving this device (`send_slash_command`, and a typed one from the composer): a
/// `/friend-code` or `/redeem <code>` is noted, so the one answer it asks for is taken.
pub(crate) fn note_sent(gs: &mut GuiState, command: &str) {
    let first = command.split_whitespace().next().unwrap_or("");
    if first.eq_ignore_ascii_case("/friend-code") {
        gs.protected.code_asked_at.push(std::time::Instant::now());
    } else if first.eq_ignore_ascii_case("/redeem") {
        gs.protected.redeem_asked_at.push(std::time::Instant::now());
    }
}

/// Whether an answer was asked for in the last [`ANSWER_WINDOW`]: asks older than that are
/// forgotten, and the oldest one still waiting is used up by this answer (one answer per ask).
fn asked(asks: &mut Vec<std::time::Instant>) -> bool {
    asks.retain(|t| t.elapsed() < ANSWER_WINDOW);
    if asks.is_empty() {
        return false;
    }
    asks.remove(0);
    true
}

/// A friend code as the server makes one: letters, digits and dashes, at most 32. Anything else is
/// not drawn at all.
fn code_shaped(code: &str) -> bool {
    !code.is_empty() && code.len() <= 32 && code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
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
            if !asked(&mut gs.protected.code_asked_at) {
                log::warn!("Dropped a friend_code_response this device did not ask for.");
                return true;
            }
            let code = frame.get("code").and_then(|c| c.as_str()).unwrap_or("").trim().to_string();
            if code_shaped(&code) {
                // The code is the server's: its line is marked as the server's words.
                let line = code_line(&code);
                gs.server_settings_status = humanity_core::i18n::tf("chat.from_server", &[("text", &line)]);
                server_line(gs, &line);
            } else {
                log::warn!("Dropped a friend code that is not shaped like one.");
            }
            true
        }
        Some("friend_code_result") => {
            if !asked(&mut gs.protected.redeem_asked_at) {
                // Never follow a key the server names unasked.
                log::warn!("Dropped a friend_code_result this device did not ask for.");
                gs.protected.code_redeemed = false;
                return true;
            }
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
        server_line(gs, &format!("The friend code did not work: {why}"));
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
        // The name is the server's to give: marked as its words.
        server_line(gs, &format!("Friend code accepted: you now follow {name}. You are friends once they follow you back."));
    }
}

/// One line from the app in the chat, where the server's own private notices appear (never
/// into an open direct message or group, where it would vanish on reload).
/// The words are an [`crate::gui::AppVoice`]: an i18n key and typed arguments, never a string
/// (the eighth review: the app's voice is a type no frame's text can reach).
pub(crate) fn system_line(gs: &mut GuiState, voice: crate::gui::AppVoice) {
    let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    let channel = crate::gui::pages::chat::notice_channel(&gs.chat_active_channel);
    let server = crate::gui::pages::chat::norm_server_url(&gs.server_url);
    gs.chat_messages.push(crate::gui::app_row(voice, channel, server, now_ms));
}

/// A line the SERVER wrote (a `system` frame, which is also how its private lines arrive, and the
/// server's own words in answer to a check), in the chat under the sender "From this server", never
/// as the app's own "System" voice (the owner branch's sixth review, H1 (d), 2026-11-21: a relay
/// wrote "Your fingerprint is <another key's>..." into the chat as a system line, drawn exactly like
/// the app's own). The website's twin is app.js `addServerMessage`.
pub(crate) fn server_line(gs: &mut GuiState, text: &str) {
    let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    gs.chat_messages.push(crate::gui::ChatMessage {
        sender_name: humanity_core::i18n::t("chat.from_server_sender").to_string(),
        content: text.to_string(),
        timestamp: crate::gui::pages::chat::format_timestamp(now_ms),
        timestamp_ms: now_ms,
        // Never into an open P2P group or DM (it would vanish on reload).
        channel: crate::gui::pages::chat::notice_channel(&gs.chat_active_channel),
        server: crate::gui::pages::chat::norm_server_url(&gs.server_url),
        origin: crate::gui::LineOrigin::Server,
        ..Default::default()
    });
}

#[cfg(test)]
#[path = "friend_code_tests.rs"]
mod tests;
