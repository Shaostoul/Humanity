//! Data-driven roles (v0.239 — Phase R1).
//!
//! Replaces the hardcoded 5-string role ladder with a `roles` table the
//! operator can extend. Each role carries a capability set
//! (can_stream / can_upload / can_voice) and a `base_tier` declaring
//! which of the existing `server_settings` per-tier numeric limits it
//! inherits. See `docs/design/roles-system.md` for the full model.
//!
//! Assignment is unchanged — `user_roles (public_key, role)` already
//! stores an arbitrary role id string. Unknown / deleted role ids
//! resolve to the `unverified` seed (default-deny).

use super::Storage;
use rusqlite::params;
use serde::{Deserialize, Serialize};

/// One role definition. Serialized over the WS protocol as `role_list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleDef {
    pub id: String,
    pub label: String,
    /// Badge color, hex string e.g. "#4FC3F7".
    pub color: String,
    /// Ordering — higher = more trusted. Used for "can't act on a
    /// higher-trust user" checks + sensible dropdown ordering.
    pub trust_level: i64,
    /// Seed role — cannot be deleted; id/trust locked (caps still editable).
    pub built_in: bool,
    pub can_stream: bool,
    pub can_upload: bool,
    pub can_voice: bool,
    /// Per-role image-attachment capability (v0.261). Effective image
    /// sharing = server_settings.image_sharing_enabled AND
    /// role.can_image_share — same master∧capability model as streaming.
    /// `#[serde(default)]` → false for any payload missing it; the DB
    /// migration sets EXISTING roles to true so upgrade is non-breaking.
    #[serde(default)]
    pub can_image_share: bool,
    /// Per-role non-image-file capability (v0.261). Same model.
    #[serde(default)]
    pub can_file_share: bool,
    /// Which `server_settings` limit tier this role inherits numeric
    /// limits from. One of "unverified" | "verified" | "mod" | "admin".
    pub base_tier: String,
    pub sort_order: i64,
}

impl Default for RoleDef {
    /// The safe default-deny role (matches the `unverified` seed). Used
    /// when a role id isn't found so a deleted/unknown role can never
    /// accidentally grant a capability.
    fn default() -> Self {
        Self {
            id: "unverified".into(),
            label: "Unverified".into(),
            color: "#9E9E9E".into(),
            trust_level: 0,
            built_in: true,
            can_stream: false,
            can_upload: false,
            can_voice: false,
            // Default-deny for an unknown/deleted role — consistent with
            // the other caps. (Existing real roles are migrated to true
            // so the upgrade itself is non-breaking; this default only
            // applies to the unresolvable-role fallback.)
            can_image_share: false,
            can_file_share: false,
            base_tier: "unverified".into(),
            sort_order: 0,
        }
    }
}

impl Storage {
    // The `roles` table is CREATEd + seeded by the startup migration in
    // storage/mod.rs (inlined there because that runs inside the
    // migration's own `conn` closure). This module owns the CRUD +
    // `role_def` resolution on top of it.

    /// All roles, ordered by sort_order then trust_level.
    pub fn list_roles(&self) -> Result<Vec<RoleDef>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id,label,color,trust_level,built_in,can_stream,can_upload,can_voice,base_tier,sort_order,can_image_share,can_file_share
                 FROM roles ORDER BY sort_order ASC, trust_level ASC",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok(RoleDef {
                    id: r.get(0)?,
                    label: r.get(1)?,
                    color: r.get(2)?,
                    trust_level: r.get(3)?,
                    built_in: r.get::<_, i64>(4)? != 0,
                    can_stream: r.get::<_, i64>(5)? != 0,
                    can_upload: r.get::<_, i64>(6)? != 0,
                    can_voice: r.get::<_, i64>(7)? != 0,
                    can_image_share: r.get::<_, i64>(10)? != 0,
                    can_file_share: r.get::<_, i64>(11)? != 0,
                    base_tier: r.get(8)?,
                    sort_order: r.get(9)?,
                })
            })?;
            Ok(rows.filter_map(|x| x.ok()).collect())
        })
    }

    /// Resolve a role id to its definition. Unknown / deleted ids fall
    /// back to the safe default-deny `RoleDef::default()` (unverified).
    /// An empty role string (legacy "no role") also maps to unverified.
    pub fn role_def(&self, role_id: &str) -> RoleDef {
        let lookup_id = if role_id.is_empty() { "unverified" } else { role_id };
        let res = self.with_conn(|conn| {
            conn.query_row(
                "SELECT id,label,color,trust_level,built_in,can_stream,can_upload,can_voice,base_tier,sort_order,can_image_share,can_file_share
                 FROM roles WHERE id = ?1",
                params![lookup_id],
                |r| {
                    Ok(RoleDef {
                        id: r.get(0)?,
                        label: r.get(1)?,
                        color: r.get(2)?,
                        trust_level: r.get(3)?,
                        built_in: r.get::<_, i64>(4)? != 0,
                        can_stream: r.get::<_, i64>(5)? != 0,
                        can_upload: r.get::<_, i64>(6)? != 0,
                        can_voice: r.get::<_, i64>(7)? != 0,
                        can_image_share: r.get::<_, i64>(10)? != 0,
                        can_file_share: r.get::<_, i64>(11)? != 0,
                        base_tier: r.get(8)?,
                        sort_order: r.get(9)?,
                    })
                },
            )
        });
        res.unwrap_or_default()
    }

    /// Which server_settings limit tier a role inherits. Always one of
    /// unverified/verified/mod/admin. Callers feed this into the
    /// `ServerSettings::*_for_role` lookups so a custom role's numeric
    /// limits follow its `base_tier` while its capabilities are its own.
    pub fn limit_tier_for_role(&self, role_id: &str) -> String {
        self.role_def(role_id).base_tier
    }

    /// Create or update a role. Built-in roles: id / trust_level /
    /// built_in / base_tier are LOCKED (caller must pass the existing
    /// values; we re-assert them defensively here); only capabilities,
    /// label, color, sort_order are mutable. Custom roles: fully mutable.
    /// Returns Err for an attempt to change a built-in's locked fields.
    pub fn upsert_role(&self, r: &RoleDef) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            // Is this id already a built-in?
            let existing_built_in: Option<i64> = conn.query_row(
                "SELECT built_in FROM roles WHERE id = ?1",
                params![r.id],
                |row| row.get(0),
            ).ok();
            let is_built_in = existing_built_in == Some(1);
            // Built-in: preserve locked fields regardless of payload.
            let (built_in, trust, base_tier) = if is_built_in {
                let (t, bt): (i64, String) = conn.query_row(
                    "SELECT trust_level, base_tier FROM roles WHERE id = ?1",
                    params![r.id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                (1_i64, t, bt)
            } else {
                (0_i64, r.trust_level, r.base_tier.clone())
            };
            conn.execute(
                "INSERT INTO roles
                   (id,label,color,trust_level,built_in,can_stream,can_upload,can_voice,base_tier,sort_order,can_image_share,can_file_share)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
                 ON CONFLICT(id) DO UPDATE SET
                   label=?2, color=?3, trust_level=?4, built_in=?5,
                   can_stream=?6, can_upload=?7, can_voice=?8,
                   base_tier=?9, sort_order=?10,
                   can_image_share=?11, can_file_share=?12",
                params![
                    r.id, r.label, r.color, trust, built_in,
                    r.can_stream as i64, r.can_upload as i64, r.can_voice as i64,
                    base_tier, r.sort_order,
                    r.can_image_share as i64, r.can_file_share as i64,
                ],
            )?;
            Ok(())
        })
    }

    /// Delete a custom role. Built-in roles are protected (Ok(false)).
    /// Users still holding the deleted id silently fall back to
    /// unverified via `role_def`, so no user_roles rewrite is needed.
    pub fn delete_role(&self, role_id: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let bi: Option<i64> = conn.query_row(
                "SELECT built_in FROM roles WHERE id = ?1",
                params![role_id],
                |row| row.get(0),
            ).ok();
            if bi == Some(1) {
                return Ok(false); // protected
            }
            let n = conn.execute("DELETE FROM roles WHERE id = ?1", params![role_id])?;
            Ok(n > 0)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_db() -> Storage {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_roles_{pid}_{nanos}.db"));
        Storage::open(&path).expect("open test db")
    }

    /// v0.261 non-breaking guarantee: every seeded built-in must have
    /// can_image_share AND can_file_share = true, so the upgrade does
    /// NOT newly deny sharing (it stays gated only by the server-wide
    /// master toggle exactly as before).
    #[test]
    fn migration_seeds_builtins_with_sharing_on() {
        let db = fresh_db();
        let roles = db.list_roles().expect("list");
        assert!(roles.iter().any(|r| r.id == "unverified"), "seeds present");
        for r in &roles {
            assert!(
                r.can_image_share && r.can_file_share,
                "built-in {} must seed sharing ON (non-breaking)", r.id
            );
        }
    }

    /// Per-role image/file caps round-trip through upsert/list/role_def
    /// — guards the appended positional SQL (?11/?12, get(10)/get(11)).
    #[test]
    fn custom_role_sharing_caps_roundtrip() {
        let db = fresh_db();
        let mut r = RoleDef::default();
        r.id = "family".into();
        r.label = "Family".into();
        r.built_in = false;
        r.base_tier = "verified".into();
        r.can_image_share = true;
        r.can_file_share = false; // deliberately asymmetric
        db.upsert_role(&r).expect("upsert");

        let got = db.role_def("family");
        assert_eq!(got.can_image_share, true, "image cap persisted");
        assert_eq!(got.can_file_share, false, "file cap persisted (no bleed)");
        // Confirm via list_roles too (different SELECT path).
        let listed = db.list_roles().unwrap();
        let fam = listed.iter().find(|x| x.id == "family").expect("in list");
        assert!(fam.can_image_share && !fam.can_file_share);

        // Flip + re-upsert → ON CONFLICT path.
        r.can_image_share = false;
        r.can_file_share = true;
        db.upsert_role(&r).unwrap();
        let got2 = db.role_def("family");
        assert!(!got2.can_image_share && got2.can_file_share);
    }

    /// Unknown / deleted role must default-DENY sharing (consistent
    /// with can_stream/upload/voice = false in RoleDef::default()).
    #[test]
    fn unknown_role_defaults_deny_sharing() {
        let db = fresh_db();
        let rd = db.role_def("does-not-exist");
        assert!(!rd.can_image_share && !rd.can_file_share);
        assert_eq!(rd.id, "unverified", "falls back to safe default-deny");
    }
}
