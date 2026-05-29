//! P2P groups — sovereign data model (Phase 1 of `docs/design/p2p-groups.md`).
//!
//! A group is NOT a server row. It is a set of **signed objects** (authority lives
//! in `signed_objects`; these tables are a fast-read PROJECTION the relay caches as
//! an *optional accelerator*). The same objects replicate peer-to-peer over WebRTC
//! DataChannels, so a group survives any single relay going down.
//!
//! ## Object-format spec (the cross-language contract — web / native / relay)
//!
//! All three are `crate::relay::core::object::Object` (canonical CBOR + BLAKE3
//! `object_id` + Dilithium3 signature). Build with `ObjectBuilder`; verify with
//! `put_signed_object` (rejects bad signatures before anything is projected).
//!
//! ### `group_v1` — the group identity
//! - `object_type = "group_v1"`
//! - `author_public_key` = the creator's Dilithium3 pubkey (the bootstrap admin)
//! - `payload` (canonical CBOR map): `{ "name": text }`
//! - **`group_id` = this object's `object_id`** (self-certifying; no server mints it).
//!
//! ### `group_member_v1` — an append-only admit/remove entry
//! - `object_type = "group_member_v1"`
//! - `references = [ group_id ]`
//! - `payload`: `{ "action": "admit" | "remove", "subject": bytes(Dilithium pubkey) }`
//! - `author_public_key` = the key performing the action. **Phase 1: must be the
//!   group creator** (multi-admin delegation via the membership-log fold is a later
//!   refinement). The current roster = the fold of these entries.
//!
//! ### `group_msg_v1` — a group message (Phase 2; spec'd here, NOT yet projected)
//! - `object_type = "group_msg_v1"`, `references = [ group_id ]`
//! - `payload_encoding = "xchacha20poly1305_v1"`, `payload` = ciphertext under the
//!   current epoch group key. Author must be an active roster member to be accepted.
//!
//! ### Connection ticket (the invite — NOT a relay URL, NOT stored here)
//! A signed blob (QR / clipboard): `{ group_id, group_name, admit_pubkey,
//! bootstrap:[{member_pubkey, kyber_pub, relays}], invite_secret, expires_at,
//! uses_remaining }`. Lets a joiner verify the group, request admission, and
//! bootstrap a peer connection. Hardened (expiry + use-limit) unlike the legacy
//! 6-hex relay code.
//!
//! ## Phase 1 scope / limitations (documented on purpose)
//! - Only the creator can admit/remove (sole bootstrap admin). Delegated admins =
//!   later refinement of the fold.
//! - Out-of-order arrival: if a `group_member_v1` is seen before its `group_v1`,
//!   it is not projected yet (the signed object is still stored; a re-index pass
//!   reconciles). Eventual, not immediate — acceptable for Phase 1.

use rusqlite::{params, OptionalExtension};

use super::{Storage, now_millis};
use super::signed_objects::author_fingerprint;
use crate::relay::core::object::Object;

/// Read a CBOR text field from an object payload (matches the per-module helper
/// pattern used across storage/*.rs).
fn read_text(object: &Object, field: &str) -> Option<String> {
    let value = crate::relay::core::encoding::from_canonical_bytes(&object.payload).ok()?;
    if let ciborium::Value::Map(entries) = value {
        for (k, v) in entries {
            if let (ciborium::Value::Text(name), ciborium::Value::Text(s)) = (k, v) {
                if name == field {
                    return Some(s);
                }
            }
        }
    }
    None
}

/// Read a CBOR bytes field from an object payload.
fn read_bytes(object: &Object, field: &str) -> Option<Vec<u8>> {
    let value = crate::relay::core::encoding::from_canonical_bytes(&object.payload).ok()?;
    if let ciborium::Value::Map(entries) = value {
        for (k, v) in entries {
            if let (ciborium::Value::Text(name), ciborium::Value::Bytes(b)) = (k, v) {
                if name == field {
                    return Some(b);
                }
            }
        }
    }
    None
}

/// Read a CBOR unsigned-integer field from an object payload.
fn read_uint(object: &Object, field: &str) -> Option<u64> {
    let value = crate::relay::core::encoding::from_canonical_bytes(&object.payload).ok()?;
    if let ciborium::Value::Map(entries) = value {
        for (k, v) in entries {
            if let (ciborium::Value::Text(name), ciborium::Value::Integer(i)) = (k, v) {
                if name == field {
                    let raw: i128 = i.into();
                    if (0..=u64::MAX as i128).contains(&raw) {
                        return Some(raw as u64);
                    }
                }
            }
        }
    }
    None
}

/// A member of a P2P group (active roster entry).
#[derive(Debug, Clone)]
pub struct P2pGroupMember {
    pub member_fp: String,
    pub member_pubkey: Vec<u8>,
}

impl Storage {
    /// Project a `group_v1` object into the `p2p_groups` table and seed the
    /// creator as the first active roster member. No-op for other object types.
    /// Called from `put_signed_object` after signature verification.
    pub fn index_group(&self, object: &Object) -> Result<bool, rusqlite::Error> {
        if object.object_type != "group_v1" {
            return Ok(false);
        }
        let name = match read_text(object, "name") {
            Some(n) if !n.trim().is_empty() => n,
            _ => return Ok(false), // malformed group object — ignore
        };
        let group_id = match object.object_id() {
            Ok(id) => id.to_hex(),
            Err(_) => return Ok(false),
        };
        let creator_pubkey = object.author_public_key.clone();
        let creator_fp = author_fingerprint(&creator_pubkey);
        let created_at = object.created_at.map(|t| t as i64);
        let now = now_millis() as i64;

        self.with_conn(|conn| {
            conn.execute(
                "INSERT OR IGNORE INTO p2p_groups
                   (group_id, name, creator_fp, creator_pubkey, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![group_id, name, creator_fp, creator_pubkey, created_at],
            )?;
            // Creator is always an active member.
            conn.execute(
                "INSERT OR IGNORE INTO p2p_group_roster
                   (group_id, member_fp, member_pubkey, active, updated_at)
                 VALUES (?1, ?2, ?3, 1, ?4)",
                params![group_id, creator_fp, creator_pubkey, now],
            )?;
            Ok(true)
        })
    }

    /// Project a `group_member_v1` admit/remove entry into `p2p_group_roster`.
    /// Phase 1: only the group creator is authorized to admit/remove; entries
    /// signed by anyone else are ignored. No-op for other object types.
    pub fn index_group_member(&self, object: &Object) -> Result<bool, rusqlite::Error> {
        if object.object_type != "group_member_v1" {
            return Ok(false);
        }
        let group_id = match object.references.first() {
            Some(g) => g.clone(),
            None => return Ok(false),
        };
        let action = match read_text(object, "action") {
            Some(a) => a,
            None => return Ok(false),
        };
        let subject = match read_bytes(object, "subject") {
            Some(s) if !s.is_empty() => s,
            _ => return Ok(false),
        };

        // Authorization (Phase 1):
        //   • the group CREATOR may admit or remove anyone, OR
        //   • ANY member may remove THEMSELVES (a self-leave: action="remove"
        //     with subject == author). You can always leave a group you're in.
        // If the group isn't projected yet (out-of-order arrival), skip — the
        // signed object is still persisted and a later re-index reconciles.
        let creator_pubkey: Option<Vec<u8>> = self.with_conn(|conn| {
            conn.query_row(
                "SELECT creator_pubkey FROM p2p_groups WHERE group_id = ?1",
                params![group_id],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()
        })?;
        let creator_pubkey = match creator_pubkey {
            Some(pk) => pk,
            None => return Ok(false), // unknown group yet
        };
        let is_creator = object.author_public_key == creator_pubkey;
        let is_self_leave = action == "remove" && subject == object.author_public_key;
        if !is_creator && !is_self_leave {
            return Ok(false); // unauthorized admit/remove — ignore
        }

        let subject_fp = author_fingerprint(&subject);
        let active: i64 = if action == "admit" { 1 } else if action == "remove" { 0 } else { return Ok(false) };
        let now = now_millis() as i64;

        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO p2p_group_roster
                   (group_id, member_fp, member_pubkey, active, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(group_id, member_fp) DO UPDATE SET
                   active = excluded.active,
                   updated_at = excluded.updated_at",
                params![group_id, subject_fp, subject, active, now],
            )?;
            Ok(true)
        })
    }

    /// Project a creator-signed `group_invite_v1` capability into
    /// `p2p_group_invites`. The author MUST be the group creator (Phase 1).
    /// No-op for other object types.
    pub fn index_group_invite(&self, object: &Object) -> Result<bool, rusqlite::Error> {
        if object.object_type != "group_invite_v1" {
            return Ok(false);
        }
        let group_id = match object.references.first() {
            Some(g) => g.clone(),
            None => return Ok(false),
        };
        let secret_hash = match read_bytes(object, "secret_hash") {
            Some(s) if s.len() == 32 => s,
            _ => return Ok(false),
        };
        let expires_at = match read_uint(object, "expires_at") {
            Some(e) => e as i64,
            None => return Ok(false),
        };
        let invite_id = match object.object_id() {
            Ok(id) => id.to_hex(),
            Err(_) => return Ok(false),
        };

        // Authorization: only the group creator may issue invites (Phase 1).
        let creator_pubkey: Option<Vec<u8>> = self.with_conn(|conn| {
            conn.query_row(
                "SELECT creator_pubkey FROM p2p_groups WHERE group_id = ?1",
                params![group_id],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()
        })?;
        let creator_pubkey = match creator_pubkey {
            Some(pk) => pk,
            None => return Ok(false), // unknown group yet
        };
        if object.author_public_key != creator_pubkey {
            return Ok(false); // only the creator may invite
        }

        let created_at = object.created_at.map(|t| t as i64);
        self.with_conn(|conn| {
            conn.execute(
                "INSERT OR IGNORE INTO p2p_group_invites
                   (invite_id, group_id, secret_hash, expires_at, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![invite_id, group_id, secret_hash, expires_at, created_at],
            )?;
            Ok(true)
        })
    }

    /// Project a `group_join_v1`: a joiner self-admits by revealing the invite
    /// secret. Valid iff the referenced invite is creator-signed (already
    /// established when the invite was projected), targets this group, has not
    /// expired, and `BLAKE3(revealed_secret)` matches the invite's commitment.
    /// On success the JOIN AUTHOR becomes an active roster member — no creator
    /// needs to be online. No-op for other object types.
    pub fn index_group_join(&self, object: &Object) -> Result<bool, rusqlite::Error> {
        if object.object_type != "group_join_v1" {
            return Ok(false);
        }
        let group_id = match object.references.first() {
            Some(g) => g.clone(),
            None => return Ok(false),
        };
        let invite_id = match object.references.get(1) {
            Some(i) => i.clone(),
            None => return Ok(false),
        };
        let secret = match read_bytes(object, "secret") {
            Some(s) if !s.is_empty() => s,
            _ => return Ok(false),
        };

        let invite: Option<(Vec<u8>, i64)> = self.with_conn(|conn| {
            conn.query_row(
                "SELECT secret_hash, expires_at FROM p2p_group_invites
                 WHERE invite_id = ?1 AND group_id = ?2",
                params![invite_id, group_id],
                |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()
        })?;
        let (secret_hash, expires_at) = match invite {
            Some(x) => x,
            None => return Ok(false), // unknown/foreign invite
        };
        let now = now_millis() as i64;
        if now >= expires_at {
            return Ok(false); // invite expired
        }
        // Constant-importance check: the revealed secret must match the
        // creator's commitment (stops anyone who lacks the ticket from joining).
        if blake3::hash(&secret).as_bytes()[..] != secret_hash[..] {
            return Ok(false);
        }

        let member_fp = author_fingerprint(&object.author_public_key);
        let member_pubkey = object.author_public_key.clone();
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO p2p_group_roster
                   (group_id, member_fp, member_pubkey, active, updated_at)
                 VALUES (?1, ?2, ?3, 1, ?4)
                 ON CONFLICT(group_id, member_fp) DO UPDATE SET
                   active = 1, updated_at = excluded.updated_at",
                params![group_id, member_fp, member_pubkey, now],
            )?;
            Ok(true)
        })
    }

    /// Project a `group_disband_v1`: the creator tears the whole group down.
    /// Authority = must be signed by the group creator. Sets `disbanded = 1`
    /// so the group drops off every member's list. The signed object is the
    /// durable tombstone (replicates P2P); re-indexing `group_v1` later won't
    /// resurrect it because `index_group` uses INSERT OR IGNORE (it never
    /// clears an existing row's disbanded flag). No-op for other object types.
    pub fn index_group_disband(&self, object: &Object) -> Result<bool, rusqlite::Error> {
        if object.object_type != "group_disband_v1" {
            return Ok(false);
        }
        let group_id = match object.references.first() {
            Some(g) => g.clone(),
            None => return Ok(false),
        };
        // Only the group creator may disband.
        let creator_pubkey: Option<Vec<u8>> = self.with_conn(|conn| {
            conn.query_row(
                "SELECT creator_pubkey FROM p2p_groups WHERE group_id = ?1",
                params![group_id],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()
        })?;
        match creator_pubkey {
            Some(pk) if pk == object.author_public_key => {}
            _ => return Ok(false), // unknown group, or not the creator
        }
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE p2p_groups SET disbanded = 1 WHERE group_id = ?1",
                params![group_id],
            )?;
            Ok(true)
        })
    }

    /// The active roster of a P2P group (the fold of its membership log).
    pub fn p2p_group_roster(&self, group_id: &str) -> Result<Vec<P2pGroupMember>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT member_fp, member_pubkey FROM p2p_group_roster
                 WHERE group_id = ?1 AND active = 1
                 ORDER BY member_fp",
            )?;
            let rows = stmt.query_map(params![group_id], |row| {
                Ok(P2pGroupMember {
                    member_fp: row.get(0)?,
                    member_pubkey: row.get(1)?,
                })
            })?;
            rows.collect()
        })
    }

    /// The P2P groups a given member (by their Dilithium pubkey) is currently
    /// in — `(group_id, name)` pairs. Drives the client's group list.
    pub fn p2p_groups_for_member(&self, member_pubkey: &[u8]) -> Result<Vec<(String, String)>, rusqlite::Error> {
        let fp = author_fingerprint(member_pubkey);
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT g.group_id, g.name
                 FROM p2p_groups g
                 JOIN p2p_group_roster r ON r.group_id = g.group_id
                 WHERE r.member_fp = ?1 AND r.active = 1 AND g.disbanded = 0
                 ORDER BY g.name COLLATE NOCASE",
            )?;
            let rows = stmt.query_map(params![fp], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            rows.collect()
        })
    }

    /// The creator fingerprint of a P2P group (for "am I the creator?" checks
    /// that gate the disband action). None if the group isn't projected.
    pub fn p2p_group_creator_fp(&self, group_id: &str) -> Result<Option<String>, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT creator_fp FROM p2p_groups WHERE group_id = ?1",
                params![group_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
        })
    }

    /// Whether a public key is an active member of a P2P group.
    pub fn p2p_group_has_member(&self, group_id: &str, pubkey: &[u8]) -> Result<bool, rusqlite::Error> {
        let fp = author_fingerprint(pubkey);
        self.with_conn(|conn| {
            let n: i64 = conn.query_row(
                "SELECT COUNT(*) FROM p2p_group_roster
                 WHERE group_id = ?1 AND member_fp = ?2 AND active = 1",
                params![group_id, fp],
                |row| row.get(0),
            )?;
            Ok(n > 0)
        })
    }

    // ── Phase 2: E2EE group messages (the relay never decrypts) ──

    /// Project a `group_epoch_key_v1` (the sealed per-epoch group key set).
    /// Phase 1 authority: only the group creator may publish epoch keys. The
    /// relay records WHICH object holds epoch N — the sealed keys stay opaque
    /// inside the payload. No-op for other object types.
    pub fn index_group_epoch_key(&self, object: &Object) -> Result<bool, rusqlite::Error> {
        if object.object_type != "group_epoch_key_v1" {
            return Ok(false);
        }
        let group_id = match object.references.first() {
            Some(g) => g.clone(),
            None => return Ok(false),
        };
        let epoch = match read_uint(object, "epoch") {
            Some(e) => e as i64,
            None => return Ok(false),
        };
        let object_id = match object.object_id() {
            Ok(id) => id.to_hex(),
            Err(_) => return Ok(false),
        };
        // Only the group creator may set epoch keys (Phase 1).
        let creator_pubkey: Option<Vec<u8>> = self.with_conn(|conn| {
            conn.query_row(
                "SELECT creator_pubkey FROM p2p_groups WHERE group_id = ?1",
                params![group_id],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()
        })?;
        match creator_pubkey {
            Some(pk) if pk == object.author_public_key => {}
            _ => return Ok(false),
        }
        let created_at = object.created_at.map(|t| t as i64);
        self.with_conn(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO p2p_group_epochs (group_id, epoch, object_id, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![group_id, epoch, object_id, created_at],
            )?;
            Ok(true)
        })
    }

    /// Project a `group_msg_v1` (an encrypted group message) into the message
    /// log — only if its author is an active roster member. The relay stores
    /// the opaque ciphertext (in signed_objects); it cannot read it. No-op for
    /// other object types or non-member authors.
    pub fn index_group_msg(&self, object: &Object) -> Result<bool, rusqlite::Error> {
        if object.object_type != "group_msg_v1" {
            return Ok(false);
        }
        let group_id = match object.references.first() {
            Some(g) => g.clone(),
            None => return Ok(false),
        };
        if !self.p2p_group_has_member(&group_id, &object.author_public_key)? {
            return Ok(false); // only active members' messages are accepted
        }
        let epoch = read_uint(object, "epoch").unwrap_or(0) as i64;
        let object_id = match object.object_id() {
            Ok(id) => id.to_hex(),
            Err(_) => return Ok(false),
        };
        let author_fp = author_fingerprint(&object.author_public_key);
        let created_at = object.created_at.map(|t| t as i64).unwrap_or_else(|| now_millis() as i64);
        self.with_conn(|conn| {
            conn.execute(
                "INSERT OR IGNORE INTO p2p_group_messages
                   (object_id, group_id, author_fp, epoch, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![object_id, group_id, author_fp, epoch, created_at],
            )?;
            Ok(true)
        })
    }

    /// The object_id of the latest (highest-epoch) `group_epoch_key_v1` for a
    /// group, if any — the member fetches it to unseal the current key.
    pub fn p2p_group_latest_epoch_object(&self, group_id: &str) -> Result<Option<String>, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT object_id FROM p2p_group_epochs
                 WHERE group_id = ?1 ORDER BY epoch DESC LIMIT 1",
                params![group_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
        })
    }

    /// ALL `group_epoch_key_v1` object_ids for a group, oldest→newest. A member
    /// fetches every epoch they were sealed into so the FULL history decrypts:
    /// each message is encrypted under the epoch key current WHEN IT WAS SENT, so
    /// after a re-key the latest key alone cannot open pre-re-key messages — the
    /// client needs the whole set and decrypts each message under its own epoch.
    pub fn p2p_group_all_epoch_objects(&self, group_id: &str) -> Result<Vec<String>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT object_id FROM p2p_group_epochs
                 WHERE group_id = ?1 ORDER BY epoch ASC",
            )?;
            let rows = stmt.query_map(params![group_id], |row| row.get::<_, String>(0))?;
            rows.collect()
        })
    }

    /// object_ids of a group's messages, oldest→newest (capped). The caller
    /// fetches each object and decrypts client-side.
    pub fn p2p_group_message_ids(&self, group_id: &str, limit: usize) -> Result<Vec<String>, rusqlite::Error> {
        let lim = limit.clamp(1, 500) as i64;
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT object_id FROM p2p_group_messages
                 WHERE group_id = ?1 ORDER BY created_at ASC, object_id ASC LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![group_id, lim], |row| row.get::<_, String>(0))?;
            rows.collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::core::object::ObjectBuilder;
    use crate::relay::core::pq_crypto::DilithiumKeypair;
    use ciborium::Value;

    fn make_test_storage() -> Storage {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_p2pgroups_test_{pid}_{nanos}.db"));
        Storage::open(&path).expect("open test db")
    }

    fn group_obj(creator: &DilithiumKeypair, name: &str) -> Object {
        ObjectBuilder::new("group_v1")
            .created_at(1000)
            .payload_cbor(&Value::Map(vec![(Value::Text("name".into()), Value::Text(name.into()))]))
            .unwrap()
            .sign(creator)
            .unwrap()
    }

    fn member_obj(by: &DilithiumKeypair, group_id: &str, action: &str, subject_pk: &[u8]) -> Object {
        ObjectBuilder::new("group_member_v1")
            .reference(group_id)
            .created_at(1001)
            .payload_cbor(&Value::Map(vec![
                (Value::Text("action".into()), Value::Text(action.into())),
                (Value::Text("subject".into()), Value::Bytes(subject_pk.to_vec())),
            ]))
            .unwrap()
            .sign(by)
            .unwrap()
    }

    fn invite_obj(by: &DilithiumKeypair, group_id: &str, secret: &[u8], expires_at: u64) -> Object {
        let sh = blake3::hash(secret);
        ObjectBuilder::new("group_invite_v1")
            .reference(group_id)
            .created_at(1002)
            .payload_cbor(&Value::Map(vec![
                (Value::Text("expires_at".into()), Value::Integer(expires_at.into())),
                (Value::Text("secret_hash".into()), Value::Bytes(sh.as_bytes().to_vec())),
            ]))
            .unwrap()
            .sign(by)
            .unwrap()
    }

    fn join_obj(by: &DilithiumKeypair, group_id: &str, invite_id: &str, secret: &[u8]) -> Object {
        ObjectBuilder::new("group_join_v1")
            .reference(group_id)
            .reference(invite_id)
            .created_at(1003)
            .payload_cbor(&Value::Map(vec![
                (Value::Text("secret".into()), Value::Bytes(secret.to_vec())),
            ]))
            .unwrap()
            .sign(by)
            .unwrap()
    }

    fn epoch_obj(by: &DilithiumKeypair, group_id: &str, epoch: u64) -> Object {
        ObjectBuilder::new("group_epoch_key_v1")
            .reference(group_id)
            .created_at(2000)
            .payload_cbor(&Value::Map(vec![
                (Value::Text("epoch".into()), Value::Integer(epoch.into())),
            ]))
            .unwrap()
            .sign(by)
            .unwrap()
    }

    fn msg_obj(by: &DilithiumKeypair, group_id: &str, epoch: u64) -> Object {
        ObjectBuilder::new("group_msg_v1")
            .reference(group_id)
            .created_at(2001)
            .payload_cbor(&Value::Map(vec![
                (Value::Text("epoch".into()), Value::Integer(epoch.into())),
                (Value::Text("ct".into()), Value::Bytes(vec![1, 2, 3, 4])),
            ]))
            .unwrap()
            .sign(by)
            .unwrap()
    }

    #[test]
    fn group_create_seeds_creator_into_roster() {
        let db = make_test_storage();
        let creator = DilithiumKeypair::generate().unwrap();
        let g = group_obj(&creator, "research");
        let gid = g.object_id().unwrap().to_hex();

        assert!(db.put_signed_object(&g, None).unwrap());
        let roster = db.p2p_group_roster(&gid).unwrap();
        assert_eq!(roster.len(), 1, "creator should be the sole member");
        assert!(db.p2p_group_has_member(&gid, &creator.public_key()).unwrap());
    }

    #[test]
    fn creator_can_admit_and_remove_members() {
        let db = make_test_storage();
        let creator = DilithiumKeypair::generate().unwrap();
        let alice = DilithiumKeypair::generate().unwrap();
        let g = group_obj(&creator, "research");
        let gid = g.object_id().unwrap().to_hex();
        db.put_signed_object(&g, None).unwrap();

        // Creator admits Alice.
        db.put_signed_object(&member_obj(&creator, &gid, "admit", &alice.public_key()), None).unwrap();
        assert!(db.p2p_group_has_member(&gid, &alice.public_key()).unwrap());
        assert_eq!(db.p2p_group_roster(&gid).unwrap().len(), 2);

        // Creator removes Alice.
        db.put_signed_object(&member_obj(&creator, &gid, "remove", &alice.public_key()), None).unwrap();
        assert!(!db.p2p_group_has_member(&gid, &alice.public_key()).unwrap());
        assert_eq!(db.p2p_group_roster(&gid).unwrap().len(), 1, "only creator remains");
    }

    // Cross-language KAT: the canonical CBOR + object_id for a fixed group_v1
    // input. The web encoder (web/shared/canonical-cbor.js, exercised by
    // scripts/group-object-kat.mjs) MUST reproduce these exact values, or a
    // web-built group object would be unverifiable by the relay. The two GOLDEN
    // constants are duplicated in that script on purpose — editing the encoding
    // must break BOTH or neither.
    const GOLDEN_PAYLOAD_HEX: &str = "a1646e616d65696b61742d67726f7570";
    const GOLDEN_OBJECT_ID: &str =
        "c909a8dfa825419c4034608b6f6482b883c15d3cbf88d1f1c76b01fe70f7db9b";

    #[test]
    fn group_v1_canonical_kat() {
        use crate::relay::core::object::Object;
        use crate::relay::core::encoding::{to_canonical_bytes, cbor_map, cbor_text};
        use crate::relay::core::pq_crypto::{DILITHIUM_PK_LEN, DILITHIUM_SIG_LEN};

        // Deterministic, language-neutral fixed input.
        let payload = to_canonical_bytes(&cbor_map(vec![("name", cbor_text("kat-group"))])).unwrap();
        let payload_hex: String = payload.iter().map(|b| format!("{b:02x}")).collect();

        let obj = Object {
            protocol_version: 1,
            object_type: "group_v1".to_string(),
            space_id: None,
            channel_id: None,
            author_public_key: (0..DILITHIUM_PK_LEN).map(|i| (i % 256) as u8).collect(),
            created_at: Some(1000),
            references: vec![],
            payload_schema_version: 1,
            payload_encoding: "cbor_canonical_v1".to_string(),
            payload,
            signature: (0..DILITHIUM_SIG_LEN).map(|i| (i % 256) as u8).collect(),
        };
        let object_id = obj.object_id().unwrap().to_hex();
        eprintln!("GROUP_KAT payload_hex={payload_hex}");
        eprintln!("GROUP_KAT object_id={object_id}");

        assert_eq!(payload_hex, GOLDEN_PAYLOAD_HEX, "payload encoding drifted");
        assert_eq!(object_id, GOLDEN_OBJECT_ID, "canonical object_id drifted (web↔native)");
    }

    #[test]
    fn non_creator_admit_is_ignored() {
        let db = make_test_storage();
        let creator = DilithiumKeypair::generate().unwrap();
        let attacker = DilithiumKeypair::generate().unwrap();
        let mallory = DilithiumKeypair::generate().unwrap();
        let g = group_obj(&creator, "research");
        let gid = g.object_id().unwrap().to_hex();
        db.put_signed_object(&g, None).unwrap();

        // Attacker (a non-member) tries to admit Mallory — must NOT take effect,
        // even though the object's OWN signature is valid (it's the authorization
        // that fails: the author isn't the group creator).
        db.put_signed_object(&member_obj(&attacker, &gid, "admit", &mallory.public_key()), None).unwrap();
        assert!(!db.p2p_group_has_member(&gid, &mallory.public_key()).unwrap());
        assert_eq!(db.p2p_group_roster(&gid).unwrap().len(), 1, "roster unchanged");
    }

    #[test]
    fn invite_join_admits_without_creator_online() {
        let db = make_test_storage();
        let creator = DilithiumKeypair::generate().unwrap();
        let alice = DilithiumKeypair::generate().unwrap();
        let g = group_obj(&creator, "research");
        let gid = g.object_id().unwrap().to_hex();
        db.put_signed_object(&g, None).unwrap();

        let secret = [9u8; 16];
        let inv = invite_obj(&creator, &gid, &secret, 9_999_999_999_999);
        let invite_id = inv.object_id().unwrap().to_hex();
        db.put_signed_object(&inv, None).unwrap();

        // Alice joins by revealing the secret — the creator is NOT involved here.
        db.put_signed_object(&join_obj(&alice, &gid, &invite_id, &secret), None).unwrap();
        assert!(db.p2p_group_has_member(&gid, &alice.public_key()).unwrap());
        assert_eq!(db.p2p_group_roster(&gid).unwrap().len(), 2);
    }

    #[test]
    fn wrong_secret_or_expired_invite_rejects_join() {
        let db = make_test_storage();
        let creator = DilithiumKeypair::generate().unwrap();
        let alice = DilithiumKeypair::generate().unwrap();
        let bob = DilithiumKeypair::generate().unwrap();
        let g = group_obj(&creator, "research");
        let gid = g.object_id().unwrap().to_hex();
        db.put_signed_object(&g, None).unwrap();

        let secret = [9u8; 16];
        let inv = invite_obj(&creator, &gid, &secret, 9_999_999_999_999);
        let invite_id = inv.object_id().unwrap().to_hex();
        db.put_signed_object(&inv, None).unwrap();
        // Wrong secret → rejected.
        db.put_signed_object(&join_obj(&alice, &gid, &invite_id, &[0u8; 16]), None).unwrap();
        assert!(!db.p2p_group_has_member(&gid, &alice.public_key()).unwrap());

        // Expired invite → rejected even with the correct secret.
        let exp = invite_obj(&creator, &gid, &secret, 1);
        let exp_id = exp.object_id().unwrap().to_hex();
        db.put_signed_object(&exp, None).unwrap();
        db.put_signed_object(&join_obj(&bob, &gid, &exp_id, &secret), None).unwrap();
        assert!(!db.p2p_group_has_member(&gid, &bob.public_key()).unwrap());
    }

    #[test]
    fn invite_from_non_creator_is_ignored() {
        let db = make_test_storage();
        let creator = DilithiumKeypair::generate().unwrap();
        let attacker = DilithiumKeypair::generate().unwrap();
        let mallory = DilithiumKeypair::generate().unwrap();
        let g = group_obj(&creator, "research");
        let gid = g.object_id().unwrap().to_hex();
        db.put_signed_object(&g, None).unwrap();

        // Attacker issues an invite for a group they don't own → not projected.
        let secret = [9u8; 16];
        let inv = invite_obj(&attacker, &gid, &secret, 9_999_999_999_999);
        let invite_id = inv.object_id().unwrap().to_hex();
        db.put_signed_object(&inv, None).unwrap();
        // Join referencing the bogus invite → no invite row → rejected.
        db.put_signed_object(&join_obj(&mallory, &gid, &invite_id, &secret), None).unwrap();
        assert!(!db.p2p_group_has_member(&gid, &mallory.public_key()).unwrap());
    }

    fn disband_obj(by: &DilithiumKeypair, group_id: &str) -> Object {
        ObjectBuilder::new("group_disband_v1")
            .reference(group_id)
            .created_at(1004)
            .payload_cbor(&Value::Map(vec![]))
            .unwrap()
            .sign(by)
            .unwrap()
    }

    #[test]
    fn member_can_self_leave() {
        let db = make_test_storage();
        let creator = DilithiumKeypair::generate().unwrap();
        let alice = DilithiumKeypair::generate().unwrap();
        let g = group_obj(&creator, "research");
        let gid = g.object_id().unwrap().to_hex();
        db.put_signed_object(&g, None).unwrap();

        // Alice joins via invite, then leaves by removing HERSELF (subject ==
        // author) — allowed even though she is NOT the creator.
        let secret = [7u8; 16];
        let inv = invite_obj(&creator, &gid, &secret, 9_999_999_999_999);
        let invite_id = inv.object_id().unwrap().to_hex();
        db.put_signed_object(&inv, None).unwrap();
        db.put_signed_object(&join_obj(&alice, &gid, &invite_id, &secret), None).unwrap();
        assert!(db.p2p_group_has_member(&gid, &alice.public_key()).unwrap());

        db.put_signed_object(&member_obj(&alice, &gid, "remove", &alice.public_key()), None).unwrap();
        assert!(!db.p2p_group_has_member(&gid, &alice.public_key()).unwrap(), "self-leave should drop Alice");
        // The group still exists for the creator.
        assert!(db.p2p_group_has_member(&gid, &creator.public_key()).unwrap());
        let mine = db.p2p_groups_for_member(&alice.public_key()).unwrap();
        assert!(mine.is_empty(), "left group must not appear in Alice's list");
    }

    #[test]
    fn non_creator_cannot_remove_someone_else() {
        let db = make_test_storage();
        let creator = DilithiumKeypair::generate().unwrap();
        let alice = DilithiumKeypair::generate().unwrap();
        let bob = DilithiumKeypair::generate().unwrap();
        let g = group_obj(&creator, "research");
        let gid = g.object_id().unwrap().to_hex();
        db.put_signed_object(&g, None).unwrap();
        // Admit both Alice and Bob.
        db.put_signed_object(&member_obj(&creator, &gid, "admit", &alice.public_key()), None).unwrap();
        db.put_signed_object(&member_obj(&creator, &gid, "admit", &bob.public_key()), None).unwrap();
        assert_eq!(db.p2p_group_roster(&gid).unwrap().len(), 3);

        // Alice tries to remove BOB (not herself, and she's not the creator) → ignored.
        db.put_signed_object(&member_obj(&alice, &gid, "remove", &bob.public_key()), None).unwrap();
        assert!(db.p2p_group_has_member(&gid, &bob.public_key()).unwrap(), "Bob must remain — Alice can't evict him");
    }

    #[test]
    fn creator_can_disband_group_for_everyone() {
        let db = make_test_storage();
        let creator = DilithiumKeypair::generate().unwrap();
        let alice = DilithiumKeypair::generate().unwrap();
        let g = group_obj(&creator, "research");
        let gid = g.object_id().unwrap().to_hex();
        db.put_signed_object(&g, None).unwrap();
        db.put_signed_object(&member_obj(&creator, &gid, "admit", &alice.public_key()), None).unwrap();
        assert_eq!(db.p2p_groups_for_member(&alice.public_key()).unwrap().len(), 1);
        assert_eq!(db.p2p_groups_for_member(&creator.public_key()).unwrap().len(), 1);

        // Creator disbands → the group disappears from BOTH lists.
        db.put_signed_object(&disband_obj(&creator, &gid), None).unwrap();
        assert!(db.p2p_groups_for_member(&alice.public_key()).unwrap().is_empty(), "disband hides it for members");
        assert!(db.p2p_groups_for_member(&creator.public_key()).unwrap().is_empty(), "disband hides it for the creator");
    }

    #[test]
    fn non_creator_disband_is_ignored() {
        let db = make_test_storage();
        let creator = DilithiumKeypair::generate().unwrap();
        let alice = DilithiumKeypair::generate().unwrap();
        let g = group_obj(&creator, "research");
        let gid = g.object_id().unwrap().to_hex();
        db.put_signed_object(&g, None).unwrap();
        db.put_signed_object(&member_obj(&creator, &gid, "admit", &alice.public_key()), None).unwrap();

        // Alice (a member, but not the creator) tries to disband → must NOT take effect.
        db.put_signed_object(&disband_obj(&alice, &gid), None).unwrap();
        assert_eq!(db.p2p_groups_for_member(&creator.public_key()).unwrap().len(), 1, "group must survive a non-creator disband");
    }

    #[test]
    fn epoch_key_indexed_only_from_creator() {
        let db = make_test_storage();
        let creator = DilithiumKeypair::generate().unwrap();
        let attacker = DilithiumKeypair::generate().unwrap();
        let g = group_obj(&creator, "research");
        let gid = g.object_id().unwrap().to_hex();
        db.put_signed_object(&g, None).unwrap();

        // A non-creator epoch key is ignored.
        db.put_signed_object(&epoch_obj(&attacker, &gid, 1), None).unwrap();
        assert_eq!(db.p2p_group_latest_epoch_object(&gid).unwrap(), None);

        // The creator's epoch key is indexed as the latest.
        let ek = epoch_obj(&creator, &gid, 1);
        let ek_id = ek.object_id().unwrap().to_hex();
        db.put_signed_object(&ek, None).unwrap();
        assert_eq!(db.p2p_group_latest_epoch_object(&gid).unwrap(), Some(ek_id));
    }

    #[test]
    fn all_epoch_objects_oldest_to_newest() {
        // The client fetches EVERY epoch object (oldest→newest) to decrypt the
        // full multi-epoch message history — verify the query orders correctly
        // regardless of insertion order.
        let db = make_test_storage();
        let creator = DilithiumKeypair::generate().unwrap();
        let g = group_obj(&creator, "research");
        let gid = g.object_id().unwrap().to_hex();
        db.put_signed_object(&g, None).unwrap();

        let e1 = epoch_obj(&creator, &gid, 1);
        let e2 = epoch_obj(&creator, &gid, 2);
        let e3 = epoch_obj(&creator, &gid, 3);
        let (id1, id2, id3) = (
            e1.object_id().unwrap().to_hex(),
            e2.object_id().unwrap().to_hex(),
            e3.object_id().unwrap().to_hex(),
        );
        // Insert OUT OF ORDER (2, then 1, then 3).
        db.put_signed_object(&e2, None).unwrap();
        db.put_signed_object(&e1, None).unwrap();
        db.put_signed_object(&e3, None).unwrap();

        assert_eq!(
            db.p2p_group_all_epoch_objects(&gid).unwrap(),
            vec![id1, id2, id3],
            "epochs must come back oldest→newest regardless of insert order",
        );
    }

    #[test]
    fn only_member_messages_are_logged() {
        let db = make_test_storage();
        let creator = DilithiumKeypair::generate().unwrap();
        let alice = DilithiumKeypair::generate().unwrap();
        let outsider = DilithiumKeypair::generate().unwrap();
        let g = group_obj(&creator, "research");
        let gid = g.object_id().unwrap().to_hex();
        db.put_signed_object(&g, None).unwrap();
        db.put_signed_object(&member_obj(&creator, &gid, "admit", &alice.public_key()), None).unwrap();

        let m = msg_obj(&alice, &gid, 1);
        let mid = m.object_id().unwrap().to_hex();
        db.put_signed_object(&m, None).unwrap();
        // Outsider (non-member) message must NOT be logged.
        db.put_signed_object(&msg_obj(&outsider, &gid, 1), None).unwrap();
        let cm = msg_obj(&creator, &gid, 1);
        let cmid = cm.object_id().unwrap().to_hex();
        db.put_signed_object(&cm, None).unwrap();

        let ids = db.p2p_group_message_ids(&gid, 100).unwrap();
        assert!(ids.contains(&mid), "member message should be logged");
        assert!(ids.contains(&cmid), "creator message should be logged");
        assert_eq!(ids.len(), 2, "the outsider's message must not be logged");
    }
}
