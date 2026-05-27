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

        // Authorization: the author must be the group's creator (Phase 1).
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
        if object.author_public_key != creator_pubkey {
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
}
