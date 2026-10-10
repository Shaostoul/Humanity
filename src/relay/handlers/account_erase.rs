//! Erasing an account at the relay: the steps every erase takes, and an admin erasing another
//! person's data (2026-10-10, docs/design/blocking-and-safe-mode.md 10i, which is the
//! specification: the message names and shapes below are what both clients build against).
//!
//! WHY AN ADMIN CAN ERASE SOMEONE ELSE. Until 10i only a person could erase their own data
//! (Settings > Account, `account_delete`, msg_handlers.rs `handle_account_delete`). A server
//! operator who learns a member is under 13 is expected to delete that child's information
//! (docs/reference/findings/2026-10-10-childrens-online-safety-rules.md, judgement call 7), and a
//! person who has lost their device can ask the admin to remove them. Moderators cannot: an
//! erase is not undoable, and it is the admins who answer for the server.
//!
//! ONE SET OF STEPS FOR BOTH. [`carry_out`] is the erase itself, and both handlers call it, so
//! the person's own erase and an admin's cannot drift apart: remember the erase, take the
//! account out of the shared world, delete its rows, re-send the member list, and last tell
//! the account's own clients (`account_erased`). Only the words differ ([`ErasedBy`]).
//!
//! THE MESSAGES (exact names; the clients build against these):
//! - client to relay, from an admin's or the owner's signed-in socket:
//!   `{"type":"admin_erase","target":"<their public key>","confirm_name":"<their registered name,
//!   typed>"}` ([`handle_admin_erase`]).
//! - refused with a `Private` notice and nothing changed ([`Refused`]): the sender is not an admin
//!   or the owner; the target is the sender; the target has no account here; the target is an
//!   admin or the owner; or the typed name is not exactly their registered name (trimmed).
//! - on success, to the erased account's own clients, last: `{"type":"account_erased","to",
//!   "partial","earlier":false,"by_admin":true}`, so they say an admin erased their data and
//!   disconnect as they do after their own erase.
//! - to the admin: `{"type":"admin_erase_done","name":"<the name they typed>","receipt":
//!   [["<table>",<rows>],...],"partial":<bool>}`: the same per-table counts the person's own erase
//!   reports (the tables with something erased, and any part that failed as `<table>_FAILED`).
//!
//! The log line names no key and no name, as the person's own erase's does (sign_ups.rs
//! `sign_up_logs_never_name_the_key` reads this file): the log outlives what the person was
//! promised.

use std::sync::Arc;

use crate::relay::handlers::broadcast::broadcast_full_user_list;
use crate::relay::relay::{RelayMessage, RelayState};

/// Who asked for the erase. It changes the words only, never the steps: the log line, whether
/// the person is sent the receipt sentence (their own erase; an admin's goes to the admin as
/// `admin_erase_done` instead), and `by_admin` in the `account_erased` their clients read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErasedBy {
    /// The person, from Settings > Account (`account_delete`).
    Themselves,
    /// An admin or the owner of this server (`admin_erase`).
    Admin,
}

/// The erase itself, for `key` registered as `name`, once the caller's checks have passed.
/// Both erases call this (msg_handlers.rs `handle_account_delete` and [`handle_admin_erase`]),
/// in this order, which matters:
///   1. remember the erase FIRST (sign_ups.rs `remember_erase`), for a limited time and as a
///      one-way fingerprint, so the account's other devices are not signed up again by
///      themselves (BUG-135), and so a game join racing the erase is refused under the world
///      lock instead of claiming a plot;
///   2. out of the shared world, with the plot freed in the same step (home_plots.rs
///      `leave_world_for_erase`), before the rest is erased;
///   3. every row (storage/account.rs `delete_account`), the freed plot and pieces counted with it;
///   4. the receipt sentence, to the person themselves only;
///   5. the member list re-sent, so everyone's roster drops the account;
///   6. last, `account_erased` to the account's own clients, so nothing after it refills what
///      the client clears.
///
/// Returns the receipt: every table's count, a failed part as `<table>_FAILED`.
pub async fn carry_out(state: &Arc<RelayState>, key: &str, name: &str, by: ErasedBy) -> Vec<(String, usize)> {
    crate::relay::handlers::sign_ups::remember_erase(state, key);
    let left = crate::relay::handlers::home_plots::leave_world_for_erase(state, key).await;
    let mut receipt = state.db.delete_account(key, name);
    left.add_to(&mut receipt); // the plot freed there, and the pieces that came down with it, count with the rest
    let summary: Vec<String> = erased_counts(&receipt).iter().map(|(label, n)| format!("{label}: {n}")).collect();
    // No key and no name in either line: the log outlives what the person was promised.
    match by {
        ErasedBy::Themselves => tracing::warn!("An account was erased ({})", summary.join(", ")),
        ErasedBy::Admin => tracing::warn!("An admin erased an account ({})", summary.join(", ")),
    }
    if by == ErasedBy::Themselves {
        let _ = state.broadcast_tx.send(RelayMessage::Private {
            to: key.to_string(),
            message: format!(
                "Your account and its data were erased from this server ({}). Local data on your own devices is untouched. {} Goodbye; you are welcome back any time.",
                if summary.is_empty() { "nothing was stored".to_string() } else { summary.join(", ") },
                crate::relay::handlers::sign_ups::remembered_note(state)
            ),
        });
    }
    broadcast_full_user_list(state).await;
    // `partial` when a part of the erase failed (`<table>_FAILED`): leaving is still right, but
    // the client must not say that Connect signs up again while the account partly exists.
    let _ = state.broadcast_tx.send(RelayMessage::AccountErased {
        to: key.to_string(),
        partial: is_partial(&receipt),
        earlier: false,
        by_admin: by == ErasedBy::Admin,
    });
    receipt
}

/// The receipt as it is reported, to the person in their sentence and to the admin in
/// `admin_erase_done`: the tables something was erased from, in the erase's order. A failed part
/// counts 1 (`<table>_FAILED`), so it is always listed.
pub fn erased_counts(receipt: &[(String, usize)]) -> Vec<(String, usize)> {
    receipt.iter().filter(|(_, n)| *n > 0).cloned().collect()
}

/// A part of the erase failed: the account partly exists here still.
pub fn is_partial(receipt: &[(String, usize)]) -> bool {
    receipt.iter().any(|(label, _)| label.ends_with("_FAILED"))
}

/// Why an `admin_erase` was refused. Each has its own words, so the admin knows what to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// The sender is not an admin or the owner (moderators included: an erase is not undoable).
    NotAnAdmin,
    /// The target is the sender, who has the ordinary erase in Settings for that.
    Yourself,
    /// No account is registered under the target key here.
    NoAccount,
    /// The target is an admin or the owner: the same rule as the person's own erase, so the
    /// server is never left without one by accident.
    AnAdmin,
    /// The typed name is not exactly the target's registered name.
    WrongName,
}

impl Refused {
    /// The notice the admin is sent.
    pub fn words(self) -> &'static str {
        match self {
            Refused::NotAnAdmin => "Erase refused: only an admin or the owner of this server can erase another person's data.",
            Refused::Yourself => "Erase refused: that is your own account. Erase your own data from Settings > Account instead.",
            Refused::NoAccount => "Erase refused: there is no account with that key on this server.",
            Refused::AnAdmin => "Erase refused: that person is an admin or the owner of this server. Remove their role first.",
            Refused::WrongName => "Erase refused: the typed name did not match their registered name.",
        }
    }
}

/// Whether `my_key` may erase `target`'s account, typing `confirm_name`: the target's registered
/// name when it may, or why not. Reads only; nothing is changed either way. The sender's role is
/// checked first, so somebody who is not an admin learns nothing about the target.
pub fn admin_erase_check(state: &RelayState, my_key: &str, target: &str, confirm_name: &str) -> Result<String, Refused> {
    let is_admin = |key: &str| matches!(state.db.get_role(key).unwrap_or_default().as_str(), "admin" | "owner");
    if !is_admin(my_key) {
        return Err(Refused::NotAnAdmin);
    }
    if target == my_key {
        return Err(Refused::Yourself);
    }
    let name = state.db.name_for_key(target).ok().flatten().unwrap_or_default();
    if target.is_empty() || name.is_empty() {
        return Err(Refused::NoAccount);
    }
    if is_admin(target) {
        return Err(Refused::AnAdmin);
    }
    if confirm_name.trim() != name {
        return Err(Refused::WrongName);
    }
    Ok(name)
}

/// `admin_erase` from a signed-in socket: an admin or the owner erases another person's data on
/// this server, by exactly the steps the person's own erase takes ([`carry_out`]), then is sent
/// the receipt (`admin_erase_done`). A refusal is a `Private` notice and changes nothing.
pub async fn handle_admin_erase(state: &Arc<RelayState>, my_key: &str, target: String, confirm_name: String) {
    let name = match admin_erase_check(state, my_key, &target, &confirm_name) {
        Ok(name) => name,
        Err(why) => {
            let _ = state.broadcast_tx.send(RelayMessage::Private { to: my_key.to_string(), message: why.words().to_string() });
            return;
        }
    };
    let receipt = carry_out(state, &target, &name, ErasedBy::Admin).await;
    let _ = state.broadcast_tx.send(RelayMessage::AdminEraseDone {
        to: my_key.to_string(),
        // The name they typed, which matched the registered one once trimmed.
        name: confirm_name.trim().to_string(),
        receipt: erased_counts(&receipt),
        partial: is_partial(&receipt),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 10i: who may erase whom, case by case, with the reason each refusal gives. The sender's
    /// role comes first (a moderator, either spelling, and a member are refused before anything
    /// about the target is read), then the target is not the sender, has an account here, is not
    /// an admin or the owner, and the typed name matches exactly once trimmed. Every case is
    /// checked before the test fails, so one run lists all that went wrong.
    ///
    /// Seen red 2026-10-10 with every check in `admin_erase_check` taken out (the name looked up
    /// and returned): all twelve refusals listed. And with "mod" let through as an admin: only
    /// the moderator case listed.
    #[test]
    fn the_check_refuses_everyone_it_is_not_for() {
        let db = crate::relay::storage::Storage::open_temp_dir("admin_erase_check");
        let state = RelayState::new(db);
        for (key, name, role) in [
            ("a1", "Admin", "admin"),
            ("o1", "Owner", "owner"),
            ("a2", "Other Admin", "admin"),
            ("m1", "Mod", "mod"),
            ("m2", "Moderator", "moderator"),
            ("v1", "Verified", "verified"),
            ("u1", "Member", ""),
            ("t1", "Target", ""),
        ] {
            state.db.register_name(name, key).unwrap();
            if !role.is_empty() {
                state.db.set_role(key, role).unwrap();
            }
        }
        let cases: [(&str, &str, &str, &str, Result<&str, Refused>); 14] = [
            ("a member", "u1", "t1", "Target", Err(Refused::NotAnAdmin)),
            ("a moderator", "m1", "t1", "Target", Err(Refused::NotAnAdmin)),
            ("a moderator by the long name", "m2", "t1", "Target", Err(Refused::NotAnAdmin)),
            ("a verified member", "v1", "t1", "Target", Err(Refused::NotAnAdmin)),
            ("an unknown sender", "zz", "t1", "Target", Err(Refused::NotAnAdmin)),
            ("an admin erasing themselves", "a1", "a1", "Admin", Err(Refused::Yourself)),
            ("an unknown target", "a1", "nobody", "Target", Err(Refused::NoAccount)),
            ("no target", "a1", "", "", Err(Refused::NoAccount)),
            ("another admin", "a1", "a2", "Other Admin", Err(Refused::AnAdmin)),
            ("the owner", "a1", "o1", "Owner", Err(Refused::AnAdmin)),
            ("a name in other letters", "a1", "t1", "target", Err(Refused::WrongName)),
            ("a different name", "a1", "t1", "Targe", Err(Refused::WrongName)),
            ("an admin, the name with spaces around it", "a1", "t1", "  Target  ", Ok("Target")),
            ("the owner, the exact name", "o1", "t1", "Target", Ok("Target")),
        ];
        let wrong: Vec<String> = cases
            .iter()
            .filter_map(|(case, sender, target, typed, want)| {
                let got = admin_erase_check(&state, sender, target, typed);
                let want = want.map(str::to_string);
                (got != want).then(|| format!("{case}: expected {want:?}, got {got:?}"))
            })
            .collect();
        assert!(wrong.is_empty(), "the check decided wrongly:\n{}", wrong.join("\n"));
        // Only reading: nobody's account changed.
        assert_eq!(state.db.name_for_key("t1").unwrap().as_deref(), Some("Target"));
        assert!(!state.db.erased_account_remembered("t1"));
    }

    /// 10i: the person's own erase and an admin's run the same steps, from one function, so
    /// they cannot drift apart. Read from the source: each handler calls `carry_out` with its
    /// own `ErasedBy`, and neither takes any of the steps itself.
    ///
    /// Seen red 2026-10-10 with the old body of `handle_account_delete` put back (its own
    /// remember, leave, delete and signal): "handle_account_delete does not erase through
    /// carry_out with ErasedBy::Themselves".
    #[test]
    fn both_erases_go_through_the_same_steps() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let read = |p: &str| std::fs::read_to_string(root.join(p)).unwrap().replace("\r\n", "\n");
        let body = |text: &str, start: &str| -> String {
            let from = &text[text.find(start).unwrap_or_else(|| panic!("{start} not found"))..];
            from[..from.find("\n}\n").unwrap()].to_string()
        };
        let own = body(&read("src/relay/handlers/msg_handlers.rs"), "pub async fn handle_account_delete");
        let admin = body(&read("src/relay/handlers/account_erase.rs"), "pub async fn handle_admin_erase");
        let steps = ["remember_erase(", "leave_world_for_erase(", "delete_account(", "broadcast_full_user_list(", "AccountErased"];
        for (name, src, by) in [("handle_account_delete", &own, "ErasedBy::Themselves"), ("handle_admin_erase", &admin, "ErasedBy::Admin")] {
            assert!(src.contains("carry_out(") && src.contains(by), "{name} does not erase through carry_out with {by}");
            let own_steps: Vec<&str> = steps.iter().copied().filter(|s| src.contains(s)).collect();
            assert!(own_steps.is_empty(), "{name} takes an erase step itself instead of through carry_out: {own_steps:?}");
        }
    }
}
