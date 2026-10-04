//! Who becomes a new account here: the two identify-time decisions about signing someone up.
//!
//!   - The per-IP cap on NEW identities (v0.280.0 anti-spam; `new_identity_capped`). Moved
//!     here verbatim from relay.rs's identify path on 2026-10-04, to make room there (relay.rs
//!     is held to a line budget, tests/file_size_ratchet.rs) and because it is the same
//!     question as the one below: does this identify create an account?
//!   - Accounts erased here, remembered for a limited time (BUG-135, the operator's decision
//!     of 2026-10-04; the table and its culling are storage/erased_accounts.rs). A key this
//!     relay remembers erasing is NOT signed up again by itself: its identify is answered with
//!     `account_erased` (`erased_here_frame`) and the socket is never bound, so no name is
//!     registered, no member row, no presence, nothing. The person comes back by pressing
//!     Connect (native, src/gui/pages/chat/left_panel.rs) or Enter (web, web/chat/app.js):
//!     only those send `sign_up_again` in the identify, and that forgets the entry
//!     (`erased_at_identify`). A game join from a key with an entry, on a socket bound before
//!     the erase, is refused with the existing `account_erased` reason (`refused_erased_join`,
//!     called by home_plots.rs `refused_join`). Bots (`bot_` keys) are never affected.

use crate::relay::relay::{RelayMessage, RelayState};
use std::sync::Arc;
use std::time::Instant;

/// Window and cap of the per-IP new-identity limit. Default of 5 an hour covers a household
/// or a small room onboarding together, with headroom.
const NEW_ID_WINDOW_SECS: u64 = 3600;
const NEW_ID_MAX_PER_IP: usize = 5;

/// What a connection over the cap is told before it is closed.
pub const NEW_IDENTITY_CAP_SENTENCE: &str = "Too many new accounts from this connection in the last hour. Try again later, or contact the relay operator if this looks wrong.";

/// v0.280.0 anti-spam: per-IP cap on DISTINCT NEW identities created in the last hour.
/// "New" = no prior `registered_names` row for this pubkey. Returning identities (already
/// registered) are exempt: this only deters scripted onboarding floods from a single IP.
/// True when this identify is over the cap (the caller says so and closes the socket). Tune
/// via the constants above if real traffic warrants.
pub fn new_identity_capped(state: &RelayState, client_ip: &str, public_key: &str) -> bool {
    let is_new = !state.db.pubkey_is_registered(public_key).unwrap_or(false);
    if !is_new {
        return false;
    }
    // All map mutation under the lock; a plain bool comes out, so the caller's .await never
    // holds the (std, not Send) guard.
    let blocked = {
        let mut map = state.new_identity_per_ip.lock().unwrap();
        let entry = map.entry(client_ip.to_string()).or_default();
        let now = Instant::now();
        entry.retain(|(_, when)| now.duration_since(*when).as_secs() < NEW_ID_WINDOW_SECS);
        let already_seen = entry.iter().any(|(pk, _)| pk == public_key);
        let distinct: std::collections::HashSet<&str> = entry.iter().map(|(pk, _)| pk.as_str()).collect();
        if already_seen {
            false
        } else if distinct.len() >= NEW_ID_MAX_PER_IP {
            true
        } else {
            entry.push((public_key.to_string(), now));
            false
        }
    };
    if blocked {
        tracing::warn!(
            "New-identity-per-IP cap hit for ip={}, new pubkey prefix={}",
            client_ip,
            &public_key[..public_key.len().min(16)],
        );
    }
    blocked
}

/// The identify-time decision for a key whose possession was just proven (the Dilithium
/// challenge verified). True: this relay remembers the key erasing its account here, and the
/// identify did not ask to sign up again, so the caller answers with `erased_here_frame` and
/// binds nothing. False: proceed as usual. When the identify carries `sign_up_again` (only the
/// person's Connect or Enter sets it, never an automatic reconnect), the entry is forgotten
/// here and the key proceeds as a new sign-up. Runs only after the proof, so nobody else can
/// learn whether a key erased here, or forget someone else's entry.
pub fn erased_at_identify(state: &RelayState, public_key: &str, sign_up_again: bool) -> bool {
    if public_key.starts_with("bot_") || !state.db.erased_account_remembered(public_key) {
        return false;
    }
    if sign_up_again {
        match state.db.forget_erased_account(public_key) {
            Ok(_) => tracing::info!("Erased account {}… chose to sign up again here", short(public_key)),
            Err(e) => tracing::error!("erased accounts: could not forget an entry: {e}"),
        }
        return false;
    }
    tracing::info!("Erased account {}… reconnected: told, not signed up", short(public_key));
    true
}

/// The answer to an identify from a key erased here earlier: the same `account_erased` the
/// erase itself ends with (msg_handlers.rs `handle_account_delete`), with `earlier` set, so
/// the clients do what they already do for an erase: leave this server, never redial it by
/// themselves, and show the sentence that pressing Connect (or Enter) signs up again.
/// `partial` is false: the entry keeps only the day, not how the erase went.
pub fn erased_here_frame(public_key: &str) -> String {
    let msg = RelayMessage::AccountErased { to: public_key.to_string(), partial: false, earlier: true };
    serde_json::to_string(&msg).unwrap_or_default()
}

/// The erase is starting (msg_handlers.rs `handle_account_delete`, after its checks passed):
/// remember it FIRST, so a game join or an identify racing the erase is already refused (a
/// join landing between the plot's release and a later record could claim a plot for the
/// account being erased). Kept even when part of the erase then fails: the person is told to
/// erase again, and until then their other devices are not signed up again by themselves.
/// A failure to remember is logged, never fatal: the erase itself goes ahead.
pub fn remember_erase(state: &RelayState, public_key: &str) {
    if public_key.starts_with("bot_") {
        return;
    }
    if let Err(e) = state.db.remember_erased_account(public_key) {
        tracing::error!("erased accounts: could not remember an erase: {e}");
    }
}

/// A game join from a key erased here (home_plots.rs `refused_join` asks first, before
/// anything is spawned): refused privately with the reason the erase itself uses
/// ("account_erased", `ERASED_SENTENCE`), so the game shows how to come back. Reachable from a
/// socket that was bound before the erase (another device of the account, still connected);
/// a new socket of an erased key is never bound (`erased_at_identify`). True when refused.
pub async fn refused_erased_join(state: &Arc<RelayState>, my_key: &str) -> bool {
    if my_key.starts_with("bot_") || !state.db.erased_account_remembered(my_key) {
        return false;
    }
    tracing::info!("Game: join refused for an erased account {}…; nothing spawned", short(my_key));
    let denied = serde_json::json!({
        "type": "game_join_denied",
        "reason": "account_erased",
        "message": crate::ship::ship_structure::ERASED_SENTENCE,
        "chat_unaffected": true,
    });
    crate::relay::handlers::msg_handlers::send_game_private(state, my_key, &denied).await;
    true
}

/// The first characters of a key, for a log line (by characters, so a long key never splits one).
fn short(key: &str) -> String {
    key.chars().take(12).collect()
}

/// The sentence the erase receipt ends with: how long this server remembers the erase
/// (storage/erased_accounts.rs `erase_memory_sentence`, the words the person read before
/// pressing the button), with this server's real setting.
pub fn remembered_note(state: &RelayState) -> String {
    let days = state.db.get_server_settings().map(|s| s.erased_accounts_ttl_days).ok();
    crate::relay::storage::erased_accounts::erase_memory_sentence(days)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_state(tag: &str) -> Arc<RelayState> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("hum_signups_{tag}_{}_{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("test folder");
        let db = crate::relay::storage::Storage::open(&dir.join("relay.db")).expect("open test db");
        Arc::new(RelayState::new(db))
    }

    /// The identify decision, case by case: a remembered key without the field is told and
    /// not signed up; with the field it is forgotten and proceeds; another key and a bot are
    /// never affected (a bot's entry, which `remember_erase` never writes, is put in by hand
    /// here to show the bot rule holds on its own).
    ///
    /// Seen red 2026-10-04 with the `bot_` check taken out of `erased_at_identify`: "a bot is
    /// never affected".
    #[test]
    fn the_identify_decision_for_remembered_other_and_bot_keys() {
        let state = fresh_state("decide");
        remember_erase(&state, "e1e1");
        assert!(erased_at_identify(&state, "e1e1", false), "a remembered key reconnecting is told, not signed up");
        assert!(erased_at_identify(&state, "e1e1", false), "and again: an automatic reconnect never forgets it");
        assert!(!erased_at_identify(&state, "f2f2", false), "another key is never affected");
        remember_erase(&state, "bot_helper");
        assert!(!state.db.erased_account_remembered("bot_helper"), "a bot's erase is not remembered");
        state.db.remember_erased_account("bot_helper").unwrap();
        assert!(!erased_at_identify(&state, "bot_helper", false), "a bot is never affected");
        assert!(!erased_at_identify(&state, "e1e1", true), "the person's Connect proceeds as a sign-up");
        assert!(!state.db.erased_account_remembered("e1e1"), "and forgets the entry");
        assert!(!erased_at_identify(&state, "e1e1", false), "after which it is an ordinary key");
    }

    /// The frame an erased key's identify is answered with is the clients' existing
    /// `account_erased`, addressed to that key, not partial, marked `earlier`.
    ///
    /// Seen red 2026-10-04 with `earlier: false` in `erased_here_frame`: "assertion `left ==
    /// right` failed / left: Object {\"earlier\": Bool(false), ...".
    #[test]
    fn the_answer_is_the_existing_account_erased_message() {
        let v: serde_json::Value = serde_json::from_str(&erased_here_frame("abc")).unwrap();
        assert_eq!(v, serde_json::json!({ "type": "account_erased", "to": "abc", "partial": false, "earlier": true }));
    }

    /// The per-IP cap kept its behaviour through the move out of relay.rs: five distinct new
    /// keys an hour from one IP, the sixth refused, a key already counted not refused again,
    /// a registered key never counted, another IP unaffected.
    ///
    /// Seen red 2026-10-04 with the cap raised to 6: "the sixth new key is over the cap".
    #[test]
    fn the_new_identity_cap_still_counts_five_new_keys_an_hour_per_ip() {
        let state = fresh_state("cap");
        for n in 0..5 {
            assert!(!new_identity_capped(&state, "10.0.0.1", &format!("new{n}")), "new key {n} is within the cap");
        }
        assert!(new_identity_capped(&state, "10.0.0.1", "new5"), "the sixth new key is over the cap");
        assert!(!new_identity_capped(&state, "10.0.0.1", "new2"), "a key already counted is not refused");
        state.db.register_name("Known", "known_key").unwrap();
        assert!(!new_identity_capped(&state, "10.0.0.1", "known_key"), "a registered key is never counted");
        assert!(!new_identity_capped(&state, "10.0.0.2", "new6"), "another IP has its own count");
    }
}
