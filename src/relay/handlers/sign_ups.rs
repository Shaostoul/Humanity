//! Who becomes a new account here: the identify-time decisions about signing someone up.
//!
//!   - The per-IP cap on NEW identities (v0.280.0 anti-spam; `new_identity_capped`). Moved
//!     here verbatim from relay.rs's identify path on 2026-10-04, to make room there (relay.rs
//!     is held to a line budget, tests/file_size_ratchet.rs) and because it is the same
//!     question as the one below: does this identify create an account?
//!   - The member row of a key signing in (`join_as_member`), moved here from relay.rs on
//!     2026-10-04 for the same reasons; it refuses a key erased here, in the same step as it
//!     writes.
//!   - Accounts erased here, remembered for a limited time (BUG-135, the operator's decision
//!     of 2026-10-04; the table and its culling are storage/erased_accounts.rs). A key this
//!     relay remembers erasing is NOT signed up again by itself: its identify is answered with
//!     `account_erased` (`erased_here_frame`) and the socket is closed, never bound, so no
//!     name is registered, no member row, no presence, nothing (`tell_erased_and_close`). The
//!     log lines here name no key. The person comes back by pressing
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

// Placed before the erase paths below on purpose: its "Auto-joined member" line names the
// key of a NEW member, which by then is not an erased account (the log test,
// `sign_up_logs_never_name_the_key`, reads from `erased_at_identify` on).

/// The member row for a key signing in (moved here verbatim from relay.rs's identify path on
/// 2026-10-04, apart from the refusal, to keep relay.rs inside its line budget): an existing
/// member's last-seen time and name are updated; a new one joins (open server model) and
/// everyone is told. Not for bots (`bot_` keys), anonymous viewers (`viewer_`), the sample
/// and test clients (scripts/ai-sample-client.js makes RANDOM hex keys, so only their names
/// tell; operator 2026-05-12: "I successfully kicked the bots on the first try. However, once
/// I rebooted they were back") or a placeholder name.
///
/// False when this relay remembers the key erasing its account here: the join checks that in
/// the same step as it writes (`join_server_unless_erased`), so an erase that landed after the
/// identify's own check still wins. Nothing is written, and relay.rs then tells and closes
/// (`tell_erased_and_close`) before it binds anything (review of BUG-135 option 2, second
/// round, finding 8: the refusal used to come after the bind, silent, leaving the erased key
/// signed in without a member row).
pub fn join_as_member(state: &RelayState, public_key: &str, final_name: &Option<String>) -> bool {
    let nm = final_name.as_deref().unwrap_or("");
    let is_test_bot_name = nm.starts_with("AISampleBot") || nm.starts_with("TestBot") || nm.starts_with("SampleBot");
    if public_key.starts_with("bot_")
        || public_key.starts_with("viewer_")
        || is_test_bot_name
        || crate::relay::handlers::live_conns::is_placeholder_name(nm)
    {
        if is_test_bot_name {
            tracing::info!("Test-bot connection accepted (name='{nm}') — not auto-joining server_members.");
        }
        return true;
    }
    let member_name = nm; // non-placeholder means non-empty: a real chosen name
    if state.db.is_member(public_key) {
        // Already a member — update last_seen and name.
        let _ = state.db.update_last_seen(public_key);
        let _ = state.db.update_member_name(public_key, member_name);
        return true;
    }
    match state.db.join_server_unless_erased(public_key, member_name) {
        Ok(true) => {
            tracing::info!("Auto-joined member: {public_key} as '{member_name}'");
            let _ = state.broadcast_tx.send(RelayMessage::MemberJoined {
                public_key: public_key.to_string(),
                name: final_name.clone(),
                role: "member".to_string(),
            });
            true
        }
        // Nothing written: erased here, or already a member (another socket joined first).
        Ok(false) => !state.db.erased_account_remembered(public_key),
        Err(e) => {
            tracing::error!("Failed to auto-join a member: {e}");
            true
        }
    }
}

/// The identify-time decision for a key whose possession was just proven (the Dilithium
/// challenge verified). True: this relay remembers the key erasing its account here, and the
/// identify did not ask to sign up again, so the caller answers with `tell_erased_and_close`
/// and binds nothing. False: proceed as usual. When the identify carries `sign_up_again` (only
/// the person's Connect or Enter sets it, never an automatic reconnect), the entry is
/// forgotten here and the key proceeds as a new sign-up. Runs only after the proof, so nobody
/// else can learn whether a key erased here, or forget someone else's entry.
///
/// The log lines name no key, not even its first characters (review finding 2): a 48-bit
/// prefix identifies a key against any list of known keys, and a log outlives the window the
/// person was promised. They keep only that such an event happened.
pub fn erased_at_identify(state: &RelayState, public_key: &str, sign_up_again: bool) -> bool {
    if public_key.starts_with("bot_") || !state.db.erased_account_remembered(public_key) {
        return false;
    }
    if sign_up_again {
        match state.db.forget_erased_account(public_key) {
            Ok(_) => tracing::info!("An erased account chose to sign up again here"),
            Err(e) => tracing::error!("erased accounts: could not forget an entry: {e}"),
        }
        return false;
    }
    tracing::info!("An erased account reconnected: told, not signed up");
    true
}

/// How the socket teardown in relay.rs names whoever left in its log lines: the key, except
/// a key whose erase this relay remembers, which is named only as "an erased account"
/// (review of BUG-135 option 2, second round, finding 2). The erasing client closes its
/// socket a moment after the erase, and its "Peer disconnected: <key>" line put the key back
/// beside the erase's own line, which names none.
pub fn log_name_for(state: &RelayState, key: &str) -> String {
    if !key.starts_with("bot_") && state.db.erased_account_remembered(key) {
        "an erased account".to_string()
    } else {
        key.to_string()
    }
}

/// The answer to an identify from a key erased here earlier: the same `account_erased` the
/// erase itself ends with (account_erase.rs `carry_out`), with `earlier` set, so
/// the clients do what they already do for an erase: leave this server, never redial it by
/// themselves, and show the sentence that pressing Connect (or Enter) signs up again.
/// `partial` says whether the erase left anything of the account here, read from the tables
/// themselves (storage/account.rs `erase_left_rows`; review finding 16): a device that was
/// offline during an erase that did not finish is then told to erase again, as the erasing
/// device was, instead of being promised a fresh sign-up. `by_admin` is false even after an
/// admin's erase: the erased-accounts entry keeps only the day, never who asked.
pub fn erased_here_frame(state: &RelayState, public_key: &str) -> String {
    let msg = RelayMessage::AccountErased {
        to: public_key.to_string(),
        partial: state.db.erase_left_rows(public_key),
        earlier: true,
        by_admin: false,
    };
    serde_json::to_string(&msg).unwrap_or_default()
}

/// Say `erased_here_frame` on a socket that is not bound, then close it (review finding 10).
/// Leaving it open let it sit until the 30-second identify timeout, and a client that did not
/// know the message (the Tasks board) then retried every few seconds. Every client of ours
/// closes its own socket on this message anyway, so nothing waits on the relay keeping it open.
/// Used by relay.rs at the identify gate and where the name registration finds the erase.
pub async fn tell_erased_and_close<S>(ws_tx: &mut S, state: &RelayState, public_key: &str)
where
    S: futures::Sink<axum::extract::ws::Message> + Unpin,
{
    use futures::SinkExt;
    let frame = erased_here_frame(state, public_key);
    let _ = ws_tx.send(axum::extract::ws::Message::Text(frame.into())).await;
    let _ = ws_tx.close().await;
}

/// The erase is starting (account_erase.rs `carry_out`, the person's own erase or an admin's,
/// after its checks passed):
/// remember it FIRST. That is what makes the game join safe: every join checks again while it
/// holds the game world's write lock (msg_handlers.rs `handle_game_join`), and the erase takes
/// that lock only after this record (home_plots.rs `leave_world_for_erase`), so a join either
/// claimed its plot before the erase took the lock (and the erase then frees it) or sees this
/// record and is refused. The same holds for the name and the membership row on the identify
/// path (storage/erased_accounts.rs `register_name_unless_erased`). Kept even when part of the
/// erase then fails: the person is told to erase again, and until then their other devices are
/// not signed up again by themselves. A failure to remember is logged, never fatal: the erase
/// itself goes ahead.
pub fn remember_erase(state: &RelayState, public_key: &str) {
    if public_key.starts_with("bot_") {
        return;
    }
    if let Err(e) = state.db.remember_erased_account(public_key) {
        tracing::error!("erased accounts: could not remember an erase: {e}");
    }
}

/// A game join from a key erased here (home_plots.rs `refused_join` asks first, before
/// anything is spawned, and msg_handlers.rs `handle_game_join` asks again under the game
/// world's write lock): refused privately with the reason the erase itself uses
/// ("account_erased", `ERASED_SENTENCE`), so the game shows how to come back. Reachable from a
/// socket that was bound before the erase (another device of the account, still connected);
/// a new socket of an erased key is never bound (`erased_at_identify`). True when refused.
pub async fn refused_erased_join(state: &Arc<RelayState>, my_key: &str) -> bool {
    if my_key.starts_with("bot_") || !state.db.erased_account_remembered(my_key) {
        return false;
    }
    tracing::info!("Game: a join from an erased account was refused; nothing spawned");
    let denied = serde_json::json!({
        "type": "game_join_denied",
        "reason": "account_erased",
        "message": crate::ship::ship_structure::ERASED_SENTENCE,
        "chat_unaffected": true,
    });
    crate::relay::handlers::msg_handlers::send_game_private(state, my_key, &denied).await;
    true
}

/// The sentence the erase receipt ends with: how long this server remembers the erase
/// (storage/erased_accounts.rs `erase_memory_sentence`, the words the person read before
/// pressing the button), with this server's real setting.
pub fn remembered_note(state: &RelayState) -> String {
    let days = state
        .db
        .get_server_settings()
        .map(|s| s.erased_accounts_ttl_days)
        .unwrap_or(crate::relay::storage::erased_accounts::DEFAULT_TTL_DAYS);
    crate::relay::storage::erased_accounts::erase_memory_sentence(days.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_state(tag: &str) -> Arc<RelayState> {
        let db = crate::relay::storage::Storage::open_temp_dir(&format!("signups_{tag}"));
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
    /// `account_erased`, addressed to that key, marked `earlier`, and `partial` exactly when
    /// the erase left rows of the account here (review finding 16): a finished erase is
    /// `partial: false`; one whose registration survived is `partial: true`, so the device
    /// that was offline is told to erase again, as the erasing device was.
    ///
    /// Seen red 2026-10-04 with `earlier: false` in `erased_here_frame`: "assertion `left ==
    /// right` failed / left: Object {\"earlier\": Bool(false), ...". And with `partial: false` put
    /// back (as on c0c04fc39), the second half: "an unfinished erase was answered as finished /
    /// left: Bool(false) / right: true".
    #[test]
    fn the_answer_is_the_existing_account_erased_message() {
        let state = fresh_state("frame");
        let v: serde_json::Value = serde_json::from_str(&erased_here_frame(&state, "abc")).unwrap();
        assert_eq!(v, serde_json::json!({ "type": "account_erased", "to": "abc", "partial": false, "earlier": true, "by_admin": false }));
        state.db.register_name("Left", "abc").unwrap();
        let v: serde_json::Value = serde_json::from_str(&erased_here_frame(&state, "abc")).unwrap();
        assert_eq!(v["partial"], true, "an unfinished erase was answered as finished");
    }

    /// Review finding 2: the log lines about erased accounts name no key, not even its first
    /// characters (a 48-bit prefix picks a key out of any list of known keys, and the log
    /// outlives the window the person was promised). Read from the source: every log call in
    /// this file's erase paths, the erase itself (msg_handlers.rs `handle_account_delete`, and
    /// all of account_erase.rs: the steps both erases share, and an admin's erase of someone
    /// else, blocking-and-safe-mode.md 10i), what it calls to take the account out of the world
    /// (home_plots.rs `leave_world_for_erase`),
    /// and the teardown that runs when the erasing client closes its socket a moment later
    /// (relay.rs, from "Another socket for this identity is still open" to the departure),
    /// which names whoever left through `log_name_for` so an erased key is not named there.
    /// Both `tracing::info!(...)` and the bare `info!(...)` relay.rs uses are read, and a call
    /// names the key when any name in it (outside a string, or captured inside one as
    /// `{name}`) is `key` or ends in `_key`, or it calls `short(`.
    ///
    /// Seen red 2026-10-04 with four characters of the key put back into the reconnect line:
    /// "a log line about an erased account names the key: [\"tracing::info!(\\\"An erased
    /// account {} reconnected: told, not signed up\\\", &public_key[..4.min(public_key.len())])\"]".
    /// And seen red 2026-10-04 on 8695b08d4 once the scan read the erase's callees and the
    /// teardown: "a log line about an erased account names the key: [\"leave_world_for_erase:
    /// info!(\\\"Game: player {} left (entity {}): their account is being erased\\\", key,
    /// entity_id)\", \"teardown: debug!(\\\"socket {my_conn_id} for {my_key} closed; ...\\\")\",
    /// ..., \"teardown: info!(\\\"Peer disconnected: {my_key}\\\")\"]".
    #[test]
    fn sign_up_logs_never_name_the_key() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let read = |p: &str| std::fs::read_to_string(root.join(p)).unwrap().replace("\r\n", "\n");
        let own = read("src/relay/handlers/sign_ups.rs");
        let own = &own[..own.find("#[cfg(test)]").unwrap()];
        // The erase paths: everything after the per-IP cap (whose own warning, about a NEW
        // identity being refused, is not about an erased account).
        let erase_paths = &own[own.find("pub fn erased_at_identify").unwrap()..];
        let body = |src: &'static str, start: &str| -> String {
            let text = read(src);
            let from = &text[text.find(start).unwrap_or_else(|| panic!("{start} not in {src}"))..];
            from[..from.find("\n}\n").unwrap()].to_string()
        };
        let erase = body("src/relay/handlers/msg_handlers.rs", "pub async fn handle_account_delete");
        let shared = read("src/relay/handlers/account_erase.rs");
        let shared = shared.split("#[cfg(test)]").next().unwrap();
        let erase = format!("{erase}\n{shared}");
        let leave = body("src/relay/handlers/home_plots.rs", "pub async fn leave_world_for_erase");
        let relay = read("src/relay/relay.rs");
        let teardown = &relay[relay.find("// ── Another socket for this identity is still open ──").unwrap()..];
        let teardown = &teardown[..teardown.find("// Broadcast updated full user list to all clients.").unwrap()];
        let mut calls = Vec::new();
        for (region, src) in [("sign_ups", erase_paths), ("erase", &erase[..]), ("leave_world_for_erase", &leave[..]), ("teardown", teardown)] {
            let mut found = 0;
            let mut rest = src;
            loop {
                // The next log macro: `info!(` and its kin, with or without `tracing::`.
                let next = ["info!(", "warn!(", "error!(", "debug!(", "trace!("]
                    .iter()
                    .filter_map(|m| {
                        let mut from = 0;
                        while let Some(i) = rest[from..].find(m) {
                            let at = from + i;
                            let prev = rest[..at].chars().last();
                            if !prev.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                                return Some(at);
                            }
                            from = at + m.len();
                        }
                        None
                    })
                    .min();
                let Some(at) = next else { break };
                let call = &rest[at..];
                // To the parenthesis that closes the macro's own, skipping string contents
                // (a call can end in "),", as a match arm does, or hold ", " in its text).
                let open = call.find('(').unwrap();
                let (mut depth, mut in_str, mut prev, mut end) = (0i32, false, ' ', call.len());
                for (i, c) in call[open..].char_indices() {
                    match c {
                        '"' if prev != '\\' => in_str = !in_str,
                        '(' if !in_str => depth += 1,
                        ')' if !in_str => {
                            depth -= 1;
                            if depth == 0 {
                                end = open + i + 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                    prev = c;
                }
                calls.push(format!("{region}: {}", &call[..end]));
                found += 1;
                rest = &call[end..];
            }
            assert!(found >= 1, "the scan found no log call in {region}");
        }
        assert!(calls.len() >= 9, "the scan found only {} log calls: {calls:?}", calls.len());
        // The names a call uses: identifiers outside strings, and `{name}` captures inside them.
        let names = |call: &str| -> Vec<String> {
            let (mut out, mut in_str, mut word, mut prev) = (Vec::new(), false, String::new(), ' ');
            let mut capture = false;
            for c in call.chars() {
                if c == '"' && prev != '\\' {
                    in_str = !in_str;
                } else if in_str && c == '{' {
                    capture = true;
                    word.clear();
                } else if c.is_alphanumeric() || c == '_' {
                    if !in_str || capture {
                        word.push(c);
                    }
                } else {
                    if !word.is_empty() && (!in_str || capture) {
                        out.push(std::mem::take(&mut word));
                    }
                    word.clear();
                    capture = false;
                }
                prev = c;
            }
            if !word.is_empty() {
                out.push(word);
            }
            out
        };
        let naming: Vec<&String> = calls
            .iter()
            .filter(|c| c.contains("short(") || names(c).iter().any(|n| n == "key" || n.ends_with("_key")))
            .collect();
        assert!(naming.is_empty(), "a log line about an erased account names the key: {naming:?}");
    }

    /// The teardown's name for whoever left: the key, except for a key whose erase this relay
    /// remembers, which is named only as an erased account (review of BUG-135 option 2, second
    /// round, finding 2: the erasing client closes its socket right after the erase, and its
    /// "Peer disconnected: <key>" line sat next to the erase's own key-free line).
    ///
    /// Seen red 2026-10-04 with `log_name_for` giving the key always (as the teardown named
    /// it): "assertion `left == right` failed: an erased key was named in the teardown's log /
    /// left: \"c3c3\" / right: \"an erased account\"".
    #[test]
    fn the_teardown_names_an_erased_key_only_as_an_erased_account() {
        let state = fresh_state("logname");
        assert_eq!(log_name_for(&state, "c3c3"), "c3c3", "an ordinary key is named as before");
        remember_erase(&state, "c3c3");
        assert_eq!(log_name_for(&state, "c3c3"), "an erased account", "an erased key was named in the teardown's log");
    }

    /// Review of BUG-135 option 2, second round, finding 8: every write the identify path makes
    /// for a key checks the erase in the same step, and a refusal tells and closes before the
    /// socket is bound, like the identify gate. The link code (which registers the key under a
    /// name and copies the creator's role) goes through `redeem_link_code_unless_erased`; the
    /// member row goes through `join_server_unless_erased` BEFORE the peer is inserted, so a
    /// refusal there has nothing bound to take back.
    ///
    /// Seen red 2026-10-04 on 8695b08d4: "relay.rs redeems a link code without checking the
    /// erase in the same step".
    #[test]
    fn every_identify_write_checks_the_erase_and_a_refusal_closes_before_binding() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let src = std::fs::read_to_string(root.join("src/relay/relay.rs")).unwrap().replace("\r\n", "\n");
        assert!(!src.contains("state.db.redeem_link_code(code"), "relay.rs redeems a link code without checking the erase in the same step");
        // The refusal: from the call to the arm (or block) that answers it, to its closing brace.
        let refusal = |call: &str, answer: &str| -> (usize, String) {
            let at = src.find(call).unwrap_or_else(|| panic!("relay.rs does not call {call}"));
            let arm = &src[at..];
            let arm = &arm[arm.find(answer).unwrap_or_else(|| panic!("no {answer} after {call}"))..];
            (at, arm[..arm.find('}').unwrap()].to_string())
        };
        let bind = src.find("state.peers.write().await.insert(public_key.clone(), peer);").expect("the bind");
        for (call, answer) in [
            ("redeem_link_code_unless_erased(code, &public_key)", "LinkRedeemed::ErasedHere"),
            ("sign_ups::join_as_member(&state, &public_key, &final_name)", "{"),
        ] {
            let (at, arm) = refusal(call, answer);
            assert!(arm.contains("tell_erased_and_close(") && arm.contains("return;"), "a refusal from {call} does not tell and close: {arm}");
            assert!(at < bind, "{call} runs after the socket is bound, so a refusal there leaves it bound");
        }
        // And the member row itself is written only through the checked step.
        let own = std::fs::read_to_string(root.join("src/relay/handlers/sign_ups.rs")).unwrap().replace("\r\n", "\n");
        let join = &own[own.find("pub fn join_as_member").unwrap()..];
        let join = &join[..join.find("\n}\n").unwrap()];
        assert!(join.contains("join_server_unless_erased(public_key, member_name)") && !join.contains("db.join_server("), "join_as_member writes the member row without checking the erase");
    }

    /// Review finding 11: a game join checks the erase again while it holds the game world's
    /// write lock, before it claims a plot. The erase records first and takes that lock only
    /// after, so a join whose first check came before the record cannot claim a plot for the
    /// erased key. Read from the source: the order in `handle_game_join`.
    ///
    /// Seen red 2026-10-04 with the check under the lock taken out (as on c0c04fc39):
    /// "handle_game_join does not check the erase again under the world lock before
    /// assign_home".
    #[test]
    fn the_game_join_checks_the_erase_again_under_the_world_lock() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let src = std::fs::read_to_string(root.join("src/relay/handlers/msg_handlers.rs")).unwrap().replace("\r\n", "\n");
        let join = &src[src.find("pub async fn handle_game_join").unwrap()..];
        let lock = join.find("let mut world = state.game_world.write().await;").expect("the join takes the write lock");
        let assign = join.find("world.assign_home(").expect("the join claims a plot");
        let check = join[lock..].find("erased_account_remembered(my_key)").map(|i| lock + i);
        assert!(
            check.is_some_and(|c| c < assign),
            "handle_game_join does not check the erase again under the world lock before assign_home"
        );
        // And the erase records before it takes that lock: the steps both erases share
        // (account_erase.rs `carry_out`, called by the person's own erase and an admin's).
        let shared = std::fs::read_to_string(root.join("src/relay/handlers/account_erase.rs")).unwrap().replace("\r\n", "\n");
        let erase = &shared[shared.find("pub async fn carry_out").unwrap()..];
        let record = erase.find("sign_ups::remember_erase(").expect("the erase records");
        let leave = erase.find("leave_world_for_erase(").expect("the erase leaves the world");
        assert!(record < leave, "the erase takes the world lock before it records");
    }

    /// The identify path writes the name and the membership row only through the checked
    /// steps (storage/erased_accounts.rs, review finding 12), and answers a refusal there by
    /// telling and closing, as the identify gate does (finding 10).
    ///
    /// Seen red 2026-10-04 with relay.rs calling the plain `register_name` again (as on
    /// c0c04fc39): "relay.rs registers a name without checking the erase in the same step".
    #[test]
    fn the_identify_path_registers_only_through_the_checked_steps() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let src = std::fs::read_to_string(root.join("src/relay/relay.rs")).unwrap().replace("\r\n", "\n");
        assert!(!src.contains("state.db.register_name(name, &public_key)"), "relay.rs registers a name without checking the erase in the same step");
        assert!(!src.contains("state.db.join_server(&public_key"), "relay.rs makes a member row without checking the erase in the same step");
        assert!(src.contains("register_name_unless_erased(name, &public_key)"));
        // The member row: sign_ups.rs `join_as_member` (moved there 2026-10-04), checked by
        // `every_identify_write_checks_the_erase_and_a_refusal_closes_before_binding`.
        assert!(src.contains("sign_ups::join_as_member(&state, &public_key, &final_name)"));
        let gate = &src[src.find("sign_ups::erased_at_identify(").unwrap()..];
        let gate = &gate[..gate.find('}').unwrap()];
        assert!(gate.contains("tell_erased_and_close(") && gate.contains("return;"), "the identify gate leaves the refused socket open: {gate}");
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
