//! The signed actions of hide and keep (2026-11-20, docs/design/hide-and-keep.md guarantee 5;
//! docs/design/signed_moderation_logs.md). A child of moderation.rs: the same rule holds, an
//! action that arrives without a valid signed object is not carried out.
//!
//! Each of `hide_and_keep`, `held_restore`, `held_export`, `held_legal_hold` and `held_destroy`
//! is a `mod_action_v1` the actor's own app builds and signs (core `moderation::HELD_ACTIONS`).
//! The relay checks it ([`Storage::check_held_action`]), carries it out (handlers/held.rs) and
//! only then records it in `held_actions` ([`Storage::record_held_action`]) with the signed object
//! as it arrived, so every admin can see who did what and check the signature themselves.
//!
//! WHO MAY: `hide_and_keep` an admin or a moderator (the handler then applies the moderation rules
//! as for every decision); the other four only the server's OWNER (the one key holding the role
//! `owner`), and Export and Restore only when the item is sealed to that very key.
//!
//! NOT PUBLIC. These records name reports and held items and carry the actor's reason, so they are
//! kept apart from the public signed-object store: `put_signed_object` refuses a held action
//! ([`is_held_action_object`]), so none can be stored there by `POST /api/v2/objects` or arrive
//! from a linked server, and none is gossiped. The public sees only counts ([`Storage::held_counts`]).
//!
//! REPLAY. An action is good for [`HELD_ACTION_FRESH_MS`] either side of the relay's clock, only
//! from the signed-in key that signed it, only for this server (its `space_id` is this server's own
//! DID), and once (its object id is unique in `held_actions`).

use humanity_protocol::moderation::{held_target_kind, HELD_ACTIONS};
use humanity_protocol::object::Object;
use rusqlite::params;

use super::{payload_int, payload_text, NumField};
use crate::relay::storage::Storage;

/// How far a held action's own time may be from the relay's clock.
pub const HELD_ACTION_FRESH_MS: i64 = 5 * 60 * 1000;

/// A held action that checked out, ready to carry out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldAction {
    pub object_id: String,
    pub action: String,
    /// The report's number (`hide_and_keep`) or the held item's.
    pub target: i64,
    /// A legal hold's end (Unix ms), 0 for none and for every other action.
    pub until: i64,
    pub reason: String,
    /// The signer's Dilithium3 public key, hex.
    pub signer: String,
    pub signed_at: i64,
}

/// One recorded held action, as every admin is shown it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldActionRow {
    pub object_id: String,
    pub action: String,
    pub target: i64,
    pub signer_key: String,
    pub reason: String,
    pub until_ms: i64,
    pub signed_at: i64,
    pub received_at: i64,
    /// The signed object as it arrived (JSON, binary fields base64): anyone shown it can check it.
    pub object: String,
}

/// How many of each held action this server has carried out, ever: the public transparency line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct HeldCounts {
    pub held: i64,
    pub exported: i64,
    pub legal_holds: i64,
    pub destroyed: i64,
    pub restored: i64,
}

/// Whether `object` is a `mod_action_v1` carrying a held action (refused by the public store).
pub fn is_held_action_object(object: &Object) -> bool {
    object.object_type == "mod_action_v1" && payload_text(object, "action").is_some_and(|a| HELD_ACTIONS.contains(&a.as_str()))
}

impl Storage {
    /// Check the signed held action `object` sent by the signed-in `socket_key` at `now_ms`: the
    /// signature, the signer, this server, the time, the payload, that it was never used, and who
    /// may take it (the module's header). The action ready to carry out, or why not in words.
    pub fn check_held_action(&self, object: &Object, socket_key: &str, now_ms: i64) -> Result<HeldAction, String> {
        if object.object_type != "mod_action_v1" {
            return Err("this is not a signed moderation action".into());
        }
        object.verify_signature().map_err(|_| "the action's signature does not check out".to_string())?;
        let signer = hex::encode(&object.author_public_key);
        if signer != socket_key {
            return Err("the action was signed by another key than the one signed in".into());
        }
        if object.space_id.as_deref() != self.server_did().ok().as_deref() {
            return Err("the action names another server".into());
        }
        let signed_at = object.created_at.unwrap_or(0) as i64;
        if (signed_at - now_ms).abs() > HELD_ACTION_FRESH_MS {
            return Err("the action is too old or dated ahead; check this device's clock and try again".into());
        }
        let action = payload_text(object, "action").unwrap_or_default();
        let Some(kind) = held_target_kind(&action) else {
            return Err("this is not one of hide and keep's actions".into());
        };
        if payload_text(object, "target_kind").as_deref() != Some(kind) {
            return Err(format!("{action} names a {kind}"));
        }
        let target = payload_text(object, "target").and_then(|t| t.parse::<i64>().ok()).filter(|t| *t > 0).ok_or_else(|| format!("{action} names no {kind} by its number"))?;
        let reason = payload_text(object, "reason").unwrap_or_default();
        if reason.trim().is_empty() {
            return Err("an action carries its reason".into());
        }
        let until = match payload_int(object, "expires_at") {
            NumField::Absent => 0,
            NumField::Value(n) => n,
            NumField::Malformed => return Err("the action's end date cannot be read".into()),
        };
        if action != "held_legal_hold" && until != 0 {
            return Err("only a legal hold carries an end date".into());
        }
        let object_id = object.object_id().map_err(|_| "the action cannot be named".to_string())?.to_hex();
        let used: i64 = self
            .with_read_conn(|c| c.query_row("SELECT COUNT(*) FROM held_actions WHERE object_id = ?1", params![object_id], |r| r.get(0)))
            .map_err(|e| format!("the server could not read its log ({e})"))?;
        if used > 0 {
            return Err("this action was already carried out once".into());
        }
        if action == "hide_and_keep" {
            let role = self.get_role(&signer).unwrap_or_default();
            if !matches!(role.as_str(), "admin" | "owner" | "mod" | "moderator") {
                return Err("only admins and moderators hide and keep".into());
            }
        } else if self.owner_keys().unwrap_or_default() != [signer.clone()] {
            return Err("only this server's owner can do that".into());
        }
        Ok(HeldAction { object_id, action, target, until, reason, signer, signed_at })
    }

    /// Record a held action that was carried out, with the signed object as it arrived
    /// (`object_json`), at `now_ms`.
    pub fn record_held_action(&self, a: &HeldAction, object_json: &str, now_ms: i64) -> Result<(), rusqlite::Error> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO held_actions (object_id, action, target, signer_key, reason, until_ms, signed_at, received_at, object)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![a.object_id, a.action, a.target, a.signer, a.reason, a.until, a.signed_at, now_ms, object_json],
            )
            .map(|_| ())
        })
    }

    /// The latest `limit` held actions, newest first.
    pub fn held_actions(&self, limit: usize) -> Result<Vec<HeldActionRow>, rusqlite::Error> {
        self.with_read_conn(|c| {
            let mut st = c.prepare(
                "SELECT object_id, action, target, signer_key, reason, until_ms, signed_at, received_at, object FROM held_actions ORDER BY id DESC LIMIT ?1",
            )?;
            let rows = st.query_map(params![limit as i64], |r| {
                Ok(HeldActionRow {
                    object_id: r.get(0)?,
                    action: r.get(1)?,
                    target: r.get(2)?,
                    signer_key: r.get(3)?,
                    reason: r.get(4)?,
                    until_ms: r.get(5)?,
                    signed_at: r.get(6)?,
                    received_at: r.get(7)?,
                    object: r.get(8)?,
                })
            })?;
            rows.collect()
        })
    }

    /// The recorded export `object_id` of held item `id` by `signer`, and when it was received.
    pub fn held_export_grant(&self, object_id: &str, id: i64) -> Result<Option<(String, i64)>, rusqlite::Error> {
        use rusqlite::OptionalExtension;
        self.with_read_conn(|c| {
            c.query_row(
                "SELECT signer_key, received_at FROM held_actions WHERE object_id = ?1 AND action = 'held_export' AND target = ?2",
                params![object_id, id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
        })
    }

    /// How many of each held action were carried out, ever.
    pub fn held_counts(&self) -> Result<HeldCounts, rusqlite::Error> {
        self.with_read_conn(|c| {
            let mut st = c.prepare("SELECT action, COUNT(*) FROM held_actions GROUP BY action")?;
            let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
            let mut n = HeldCounts::default();
            for row in rows {
                let (action, count) = row?;
                match action.as_str() {
                    "hide_and_keep" => n.held = count,
                    "held_export" => n.exported = count,
                    "held_legal_hold" => n.legal_holds = count,
                    "held_destroy" => n.destroyed = count,
                    "held_restore" => n.restored = count,
                    _ => {}
                }
            }
            Ok(n)
        })
    }
}
