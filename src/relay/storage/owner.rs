//! The server's owner, the storage half (2026-11-20, docs/design/hide-and-keep.md, "How the owner
//! role is set"; the handler is handlers/owner.rs).
//!
//! WHO THE OWNER IS. The one key holding the role `owner` (held.rs `owner_keys`). Hide and keep
//! seals every held copy to that key's card, so the role changes hands only two ways, each here:
//! a CLAIM ([`Storage::claim_owner`]) while no key holds it, with the one-time code the relay
//! prints to its own log (handlers/owner.rs), and a HANDOVER from the living owner
//! (owner_handover.rs), which re-seals every held copy first. Nothing else grants or removes it.
//!
//! THE SUCCESSOR (the operator, 2026-11-20: "Might as well, just in case."). The owner may name ONE
//! member with a checked key card; from then on every held copy is sealed to both cards, and
//! naming, changing or removing the successor re-seals the copies the same way a handover does. The
//! successor has no power while the owner holds the role: export, legal hold, destroy and restore
//! stay the owner's alone. It matters once the successor becomes the owner, by a handover or by the
//! claim code. A claim or a handover clears the successor (the new owner names their own); copies
//! sealed to an old owner and their successor stay as they are, and open for whoever of the two
//! holds the role.
//!
//! THE GUARD. `set_role` (channels.rs) refuses to grant `owner` and refuses to change the role of
//! the key holding it ([`owner_protected`]), `ban_user` and `game_ban` refuse that key, and a
//! linked device of the owner gets `admin`, not `owner` (messages.rs). The handlers say why in
//! words first; this is the floor under them, so a path that forgets to ask still cannot demote,
//! ban or duplicate the owner (the operator, 2026-11-20: "We just want to make sure the system
//! itself can't be abused.").
//!
//! THE LOG. `owner_log` keeps one row per claim and per handover: who, when, and for a handover
//! the signed `owner_handover` object as it arrived. Never deleted; shown to admins.

use rusqlite::{params, OptionalExtension};

use super::Storage;

/// The owner's role name.
pub const OWNER_ROLE: &str = "owner";

/// What a role change that would touch the owner fails with (inside a rusqlite error, so every
/// caller's existing error path logs it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnerProtected;

impl std::fmt::Display for OwnerProtected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the server's owner role changes only by a claim or a handover, and the owner cannot be banned")
    }
}

impl std::error::Error for OwnerProtected {}

/// The error [`OwnerProtected`] travels in.
pub fn owner_protected() -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(OwnerProtected))
}

/// Whether `e` is the owner guard's refusal.
pub fn is_owner_protected(e: &rusqlite::Error) -> bool {
    matches!(e, rusqlite::Error::ToSqlConversionFailure(inner) if inner.downcast_ref::<OwnerProtected>().is_some())
}

/// A role as it is stored and compared: trimmed and in lower case, so " Owner", "OWNER" and
/// "owner" are one role everywhere (the owner review's finding 10: `set_role` refused only the
/// exact word while the apps' role helpers trimmed and lowered, so an admin could assign "Owner").
pub fn canonical_role(role: &str) -> String {
    role.trim().to_ascii_lowercase()
}

/// THE ONE WRITE OF A ROLE that is not a claim or a handover (the owner review's finding 3): every
/// path that sets someone's role (`set_role`, a link code's redeem) comes through here. The role is
/// stored canonical; `owner` in any spelling is refused, and so is any change to the row of the key
/// holding the owner role, whatever the role written. [`owner_protected`] when refused.
pub(crate) fn write_role_in(conn: &rusqlite::Connection, key: &str, role: &str) -> Result<(), rusqlite::Error> {
    let role = canonical_role(role);
    if role == OWNER_ROLE {
        return Err(owner_protected());
    }
    let changed = conn.execute(
        "INSERT INTO user_roles (public_key, role) VALUES (?1, ?2)
         ON CONFLICT(public_key) DO UPDATE SET role = ?2 WHERE lower(trim(user_roles.role)) <> 'owner'",
        params![key, role],
    )?;
    if changed == 0 {
        return Err(owner_protected());
    }
    Ok(())
}

/// One claim or handover, as admins are shown it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerLogRow {
    /// `claim` or `handover`.
    pub kind: String,
    /// The owner before (empty for a claim) and after.
    pub old_key: String,
    pub new_key: String,
    pub reason: String,
    pub at: i64,
    /// The signed `owner_handover` object as it arrived (JSON); empty for a claim.
    pub object: String,
}

impl Storage {
    /// Whether `key` holds the owner role here.
    pub fn is_owner(&self, key: &str) -> bool {
        !key.is_empty() && self.get_role(key).is_ok_and(|r| r == OWNER_ROLE)
    }

    /// Whether any key holds the owner role. A read that fails counts as yes, so a broken
    /// database never mints a claim code.
    pub fn has_owner(&self) -> bool {
        self.owner_keys().map(|k| !k.is_empty()).unwrap_or(true)
    }

    /// Make `key` the owner, at `now_ms`, if no key holds the role: in one transaction, with the
    /// claim in the log. Ok(false), and nothing changed, when someone already holds it. The code
    /// and the key card are the handler's to check (handlers/owner.rs).
    pub fn claim_owner(&self, key: &str, now_ms: i64) -> Result<bool, rusqlite::Error> {
        self.with_conn_mut(|conn| {
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            let owners: i64 = tx.query_row("SELECT COUNT(*) FROM user_roles WHERE role = 'owner'", [], |r| r.get(0))?;
            if owners > 0 {
                return Ok(false);
            }
            tx.execute(
                "INSERT INTO user_roles (public_key, role) VALUES (?1, 'owner') ON CONFLICT(public_key) DO UPDATE SET role = 'owner'",
                params![key],
            )?;
            tx.execute(
                "INSERT INTO owner_log (kind, old_key, new_key, object_id, reason, at, object) VALUES ('claim', '', ?1, NULL, '', ?2, '')",
                params![key, now_ms],
            )?;
            tx.execute("DELETE FROM owner_successor", [])?;
            tx.commit()?;
            Ok(true)
        })
    }

    /// The successor the owner named, if any.
    pub fn successor(&self) -> Option<String> {
        self.with_read_conn(|c| c.query_row("SELECT public_key FROM owner_successor WHERE id = 1", [], |r| r.get::<_, String>(0)).optional())
            .ok()
            .flatten()
            .filter(|k| !k.is_empty())
    }

    /// Whether the signed owner action `object_id` was already carried out once.
    pub fn owner_handover_used(&self, object_id: &str) -> Result<bool, rusqlite::Error> {
        self.with_read_conn(|c| {
            c.query_row("SELECT 1 FROM owner_log WHERE object_id = ?1", params![object_id], |_| Ok(()))
                .optional()
                .map(|r| r.is_some())
        })
    }

    /// The latest `limit` claims and handovers, newest first.
    pub fn owner_log(&self, limit: usize) -> Result<Vec<OwnerLogRow>, rusqlite::Error> {
        self.with_read_conn(|c| {
            let mut st = c.prepare("SELECT kind, old_key, new_key, reason, at, object FROM owner_log ORDER BY id DESC LIMIT ?1")?;
            let rows = st.query_map(params![limit as i64], |r| {
                Ok(OwnerLogRow { kind: r.get(0)?, old_key: r.get(1)?, new_key: r.get(2)?, reason: r.get(3)?, at: r.get(4)?, object: r.get(5)? })
            })?;
            rows.collect()
        })
    }
}

// The handover: every held copy re-wrapped for the new owner, then the role moves (2026-11-20).
#[path = "owner_handover.rs"]
pub mod handover;

#[cfg(test)]
#[path = "owner_tests.rs"]
mod tests;
