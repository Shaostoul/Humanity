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
    /// Per-role numeric limits (v0.262 — R4). Previously these lived as
    /// 4 per-tier columns on `server_settings` and a role merely pointed
    /// at a tier via `base_tier`; now every role OWNS its limits, so the
    /// "Per-role limits" matrix and the Roles grid are one cohesive
    /// table. Migration backfills existing roles from their old
    /// base_tier's live server_settings values (non-breaking). serde
    /// defaults = the conservative unverified tier so a missing payload
    /// can never silently grant a huge limit.
    #[serde(default = "default_max_chars")]
    pub max_chars: i64,
    #[serde(default = "default_max_upload_mb")]
    pub max_upload_mb: i64,
    #[serde(default = "default_max_uploads_kept")]
    pub max_uploads_kept: i64,
    /// LEGACY (pre-R4): which server_settings tier this role used to
    /// inherit from. No longer a runtime indirection — kept only as the
    /// migration source and as the "prefill from preset" convenience in
    /// the add-role form. One of unverified|verified|mod|admin.
    pub base_tier: String,
    pub sort_order: i64,
    /// The ship-editing rank (ship homes increment 5, 2026-10-05): people
    /// with this role build in the ship's shared spaces (the `zone:`
    /// frames: the Commons, First Street) through the server. Their own
    /// plot needs no rank. There is no server-wide switch for it. The
    /// built-in Admin role has it by default; the server's owner always
    /// does ([`Storage::has_ship_rank`]). `#[serde(default)]` → false for
    /// a payload without it (default-deny).
    #[serde(default)]
    pub can_edit_ship: bool,
    /// The largest single Media file this role may send, in MB, whichever way it is sent (one
    /// request or a resumable upload; docs/design/media-showcase.md, follow-up B1, 2026-10-11).
    /// 0 means "up to the person's Media share", the default for every role. Kept between 0 and
    /// [`MEDIA_FILE_MB_MAX`] when saved.
    #[serde(default = "limit_not_sent")]
    pub max_media_file_mb: i64,
    /// The largest file this role may send in a private conversation (a DM or a P2P group: an
    /// encrypted upload), in MB (follow-up B2). Default 6, the number both apps used before the
    /// server said. Kept between 1 and [`PRIVATE_FILE_MB_MAX`] when saved.
    #[serde(default = "limit_not_sent")]
    pub max_private_file_mb: i64,
    /// How much this role may upload in a day, in MB, over every upload route, counted by the
    /// relay (follow-up B3; the operator's answers of 2026-10-11, number 3). 0 means no limit,
    /// the built-in Admin role's default. The day is the UTC calendar day.
    #[serde(default = "limit_not_sent")]
    pub max_upload_mb_per_day: i64,
    /// How many uploads this role may start in an hour (the clock hour, UTC), over every upload
    /// route (B3). 0 means no limit, the built-in Admin role's default.
    #[serde(default = "limit_not_sent")]
    pub max_uploads_per_hour: i64,
    /// The most this role's holder may keep on the server in files from private conversations at
    /// once, in MB (follow-up B5, 2026-10-11): each leaves after the DM mailbox lifetime, and a
    /// new one past this is refused until the oldest leaves or the person removes some. 0 means
    /// no limit, the built-in Admin role's default.
    #[serde(default = "limit_not_sent")]
    pub max_private_held_mb: i64,
}

/// The most `max_media_file_mb` can be: 64 GiB, the resumable upload's sanity ceiling
/// (relay/resumable.rs `SIZE_CEILING`).
pub const MEDIA_FILE_MB_MAX: i64 = 64 * 1024;
/// The most `max_private_file_mb` can be: one request's 1 GB ceiling (upload_limits.rs).
pub const PRIVATE_FILE_MB_MAX: i64 = 1024;
/// The most `max_upload_mb_per_day` can be (a million MB, about a terabyte).
pub const UPLOAD_MB_PER_DAY_MAX: i64 = 1_000_000;
/// The most `max_uploads_per_hour` can be.
pub const UPLOADS_PER_HOUR_MAX: i64 = 100_000;
/// The most `max_private_held_mb` can be (a million MB, about a terabyte).
pub const PRIVATE_HELD_MB_MAX: i64 = 1_000_000;

impl RoleDef {
    /// The role with its upload limits held to their ranges, as it is saved: the Media file limit
    /// 0 (none) to [`MEDIA_FILE_MB_MAX`], the private file limit 1 to [`PRIVATE_FILE_MB_MAX`]
    /// (never below 1 MB, as `max_upload_mb` is where it is used), and the day's and the hour's
    /// allowances 0 (none) up to their maximums.
    pub fn with_limits_in_range(&self) -> RoleDef {
        let mut r = self.clone();
        r.max_media_file_mb = r.max_media_file_mb.clamp(0, MEDIA_FILE_MB_MAX);
        r.max_private_file_mb = r.max_private_file_mb.clamp(1, PRIVATE_FILE_MB_MAX);
        r.max_upload_mb_per_day = r.max_upload_mb_per_day.clamp(0, UPLOAD_MB_PER_DAY_MAX);
        r.max_uploads_per_hour = r.max_uploads_per_hour.clamp(0, UPLOADS_PER_HOUR_MAX);
        r.max_private_held_mb = r.max_private_held_mb.clamp(0, PRIVATE_HELD_MB_MAX);
        r
    }
}

/// What a role save carries for an upload limit its sender did not send: an app from before
/// these five limits (v0.1484.0 and older) saves a role without them, and a serde default there
/// would reset each to a default on every save (Admin's "no limit" to 100 MB a day). The save
/// keeps the stored value instead (`Storage::upsert_role`); a new role gets the default.
/// (Persisted-format review before the v0.1485.0 deploy, 2026-10-11.) The smallest i64, a value no
/// app sends: a -1 a client did send is out of range and held to the range like any other.
pub const LIMIT_NOT_SENT: i64 = i64::MIN;
fn limit_not_sent() -> i64 {
    LIMIT_NOT_SENT
}

// Conservative serde/struct defaults = the historical `unverified`
// tier numbers. Only used for the unresolvable-role fallback / a
// payload missing the field; real roles carry their own values
// (seeded or migrated from the live server_settings tier).
fn default_max_chars() -> i64 { 280 }
fn default_max_upload_mb() -> i64 { 5 }
fn default_max_uploads_kept() -> i64 { 4 }
// B2 and B3 (2026-10-11): the private file limit both apps used before the server said, and the
// Unverified tier's day and hour allowances (storage/mod.rs seeds the other built-ins).
fn default_max_private_file_mb() -> i64 { 6 }
fn default_max_upload_mb_per_day() -> i64 { 100 }
fn default_max_uploads_per_hour() -> i64 { 20 }
// B5: what the Unverified tier may hold in private files at once (five files at the 6 MB limit).
fn default_max_private_held_mb() -> i64 { 30 }

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
            max_chars: default_max_chars(),
            max_upload_mb: default_max_upload_mb(),
            max_uploads_kept: default_max_uploads_kept(),
            base_tier: "unverified".into(),
            sort_order: 0,
            can_edit_ship: false,
            max_media_file_mb: 0,
            max_private_file_mb: default_max_private_file_mb(),
            max_upload_mb_per_day: default_max_upload_mb_per_day(),
            max_uploads_per_hour: default_max_uploads_per_hour(),
            max_private_held_mb: default_max_private_held_mb(),
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
        // Read-only: SELECT + query_map (role list broadcast). Read pool.
        self.with_read_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id,label,color,trust_level,built_in,can_stream,can_upload,can_voice,base_tier,sort_order,can_image_share,can_file_share,max_chars,max_upload_mb,max_uploads_kept,can_edit_ship,
                        max_media_file_mb,max_private_file_mb,max_upload_mb_per_day,max_uploads_per_hour,max_private_held_mb
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
                    max_chars: r.get(12)?,
                    max_upload_mb: r.get(13)?,
                    max_uploads_kept: r.get(14)?,
                    base_tier: r.get(8)?,
                    sort_order: r.get(9)?,
                    can_edit_ship: r.get::<_, i64>(15)? != 0,
                    max_media_file_mb: r.get(16)?,
                    max_private_file_mb: r.get(17)?,
                    max_upload_mb_per_day: r.get(18)?,
                    max_uploads_per_hour: r.get(19)?,
                    max_private_held_mb: r.get(20)?,
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
                "SELECT id,label,color,trust_level,built_in,can_stream,can_upload,can_voice,base_tier,sort_order,can_image_share,can_file_share,max_chars,max_upload_mb,max_uploads_kept,can_edit_ship,
                        max_media_file_mb,max_private_file_mb,max_upload_mb_per_day,max_uploads_per_hour,max_private_held_mb
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
                        max_chars: r.get(12)?,
                        max_upload_mb: r.get(13)?,
                        max_uploads_kept: r.get(14)?,
                        base_tier: r.get(8)?,
                        sort_order: r.get(9)?,
                        can_edit_ship: r.get::<_, i64>(15)? != 0,
                        max_media_file_mb: r.get(16)?,
                        max_private_file_mb: r.get(17)?,
                        max_upload_mb_per_day: r.get(18)?,
                        max_uploads_per_hour: r.get(19)?,
                        max_private_held_mb: r.get(20)?,
                    })
                },
            )
        });
        res.unwrap_or_default()
    }

    /// Whether `key` holds the ship-editing rank: builds in the ship's
    /// shared spaces through the server (ship homes increment 5). The
    /// server's owner always does; anyone else when their role has
    /// `can_edit_ship` (the built-in Admin role does, by default). No role,
    /// or a role that was deleted, reads as Unverified, which has not.
    pub fn has_ship_rank(&self, key: &str) -> bool {
        let role = self.get_role(key).unwrap_or_default();
        role == "owner" || self.role_def(&role).can_edit_ship
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
    /// `r` with every upload limit its sender did not send ([`LIMIT_NOT_SENT`]) filled in: the
    /// stored role's value when the role exists, else the default a new role gets.
    fn unsent_limits_kept(&self, r: &RoleDef) -> Result<RoleDef, rusqlite::Error> {
        let mut out = r.clone();
        let stored: Option<(i64, i64, i64, i64, i64)> = self.with_conn(|conn| {
            use rusqlite::OptionalExtension;
            conn.query_row(
                "SELECT max_media_file_mb, max_private_file_mb, max_upload_mb_per_day, max_uploads_per_hour, max_private_held_mb
                 FROM roles WHERE id = ?1",
                params![r.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .optional()
        })?;
        let fresh = RoleDef::default();
        let (media, private, day, hour, held) = stored.unwrap_or((
            fresh.max_media_file_mb,
            fresh.max_private_file_mb,
            fresh.max_upload_mb_per_day,
            fresh.max_uploads_per_hour,
            fresh.max_private_held_mb,
        ));
        let keep = |sent: &mut i64, kept: i64| {
            if *sent == LIMIT_NOT_SENT {
                *sent = kept;
            }
        };
        keep(&mut out.max_media_file_mb, media);
        keep(&mut out.max_private_file_mb, private);
        keep(&mut out.max_upload_mb_per_day, day);
        keep(&mut out.max_uploads_per_hour, hour);
        keep(&mut out.max_private_held_mb, held);
        Ok(out)
    }

    pub fn upsert_role(&self, r: &RoleDef) -> Result<(), rusqlite::Error> {
        // A limit the sender did not send keeps its stored value (a new role takes the default),
        // then every limit is held to its range (B1 to B3, 2026-10-11).
        let r = &self.unsent_limits_kept(r)?.with_limits_in_range();
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
                   (id,label,color,trust_level,built_in,can_stream,can_upload,can_voice,base_tier,sort_order,can_image_share,can_file_share,max_chars,max_upload_mb,max_uploads_kept,can_edit_ship,
                    max_media_file_mb,max_private_file_mb,max_upload_mb_per_day,max_uploads_per_hour,max_private_held_mb)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21)
                 ON CONFLICT(id) DO UPDATE SET
                   label=?2, color=?3, trust_level=?4, built_in=?5,
                   can_stream=?6, can_upload=?7, can_voice=?8,
                   base_tier=?9, sort_order=?10,
                   can_image_share=?11, can_file_share=?12,
                   max_chars=?13, max_upload_mb=?14, max_uploads_kept=?15,
                   can_edit_ship=?16,
                   max_media_file_mb=?17, max_private_file_mb=?18,
                   max_upload_mb_per_day=?19, max_uploads_per_hour=?20,
                   max_private_held_mb=?21",
                params![
                    r.id, r.label, r.color, trust, built_in,
                    r.can_stream as i64, r.can_upload as i64, r.can_voice as i64,
                    base_tier, r.sort_order,
                    r.can_image_share as i64, r.can_file_share as i64,
                    r.max_chars, r.max_upload_mb, r.max_uploads_kept,
                    r.can_edit_ship as i64,
                    r.max_media_file_mb, r.max_private_file_mb,
                    r.max_upload_mb_per_day, r.max_uploads_per_hour,
                    r.max_private_held_mb,
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
        Storage::open_temp("roles")
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

    /// R4 (v0.262): per-role numeric limits round-trip through
    /// upsert/list/role_def — guards the appended ?13/?14/?15 +
    /// get(12)/get(13)/get(14) positional SQL.
    #[test]
    fn custom_role_numeric_limits_roundtrip() {
        let db = fresh_db();
        let mut r = RoleDef::default();
        r.id = "vip".into();
        r.label = "VIP".into();
        r.built_in = false;
        r.base_tier = "verified".into();
        r.max_chars = 7777;
        r.max_upload_mb = 333;
        r.max_uploads_kept = 42;
        db.upsert_role(&r).expect("upsert");

        let got = db.role_def("vip");
        assert_eq!(got.max_chars, 7777);
        assert_eq!(got.max_upload_mb, 333);
        assert_eq!(got.max_uploads_kept, 42);
        let listed = db.list_roles().unwrap();
        let v = listed.iter().find(|x| x.id == "vip").unwrap();
        assert_eq!((v.max_chars, v.max_upload_mb, v.max_uploads_kept), (7777, 333, 42));

        // Mutate + re-upsert (ON CONFLICT path).
        r.max_chars = 12;
        db.upsert_role(&r).unwrap();
        assert_eq!(db.role_def("vip").max_chars, 12);
    }

    /// R4 non-breaking: fresh-DB built-ins must seed the EXACT canonical
    /// historical per-tier numbers each role used to inherit via
    /// base_tier — so a fresh install behaves identically and the
    /// upgrade backfill (same CASE mapping) is provably correct.
    #[test]
    fn builtins_seed_canonical_r4_numbers() {
        let db = fresh_db();
        let by = |id: &str| db.role_def(id);
        let u = by("unverified");
        assert_eq!((u.max_chars, u.max_upload_mb, u.max_uploads_kept), (280, 5, 4));
        let v = by("verified");
        assert_eq!((v.max_chars, v.max_upload_mb, v.max_uploads_kept), (1000, 25, 20));
        let d = by("donor"); // base_tier = verified
        assert_eq!((d.max_chars, d.max_upload_mb, d.max_uploads_kept), (1000, 25, 20));
        let m = by("mod");
        assert_eq!((m.max_chars, m.max_upload_mb, m.max_uploads_kept), (4000, 100, 100));
        let a = by("admin");
        assert_eq!((a.max_chars, a.max_upload_mb, a.max_uploads_kept), (10000, 500, 500));
    }

    /// PRODUCTION-INCIDENT REGRESSION (2026-05-17). v0.261/v0.262
    /// seeded the roles built-ins BEFORE the idempotent ALTERs that add
    /// the new columns. Fresh DBs were fine (CREATE makes the columns)
    /// so every fresh-DB test passed — but on the LIVE relay.db
    /// `CREATE TABLE IF NOT EXISTS` was a no-op, the seed INSERT
    /// referenced a missing column, Storage::open panicked, and the
    /// relay crash-looped (502, total outage). This test reproduces the
    /// UPGRADE path the original tests never exercised: build a current
    /// DB, revert `roles` to the pre-v0.261 10-column shape with rows
    /// present, then reopen Storage. Pre-fix this panics; post-fix
    /// (ALTER-before-seed) it must open cleanly AND backfill correctly.
    #[test]
    fn upgrade_from_pre_v0261_roles_schema_does_not_panic() {
        use rusqlite::Connection;
        let path = crate::test_temp::db("rolesupg");

        // 1. Fully migrate a fresh DB (creates roles + server_settings).
        drop(Storage::open(&path).expect("fresh open"));

        // 2. Rewind `roles` to the pre-v0.261 schema with built-in rows
        //    present — exactly the live relay.db's shape on deploy day.
        {
            let c = Connection::open(&path).expect("raw open");
            c.execute_batch(
                "DROP TABLE roles;
                 CREATE TABLE roles (
                    id TEXT PRIMARY KEY, label TEXT NOT NULL, color TEXT NOT NULL,
                    trust_level INTEGER NOT NULL, built_in INTEGER NOT NULL,
                    can_stream INTEGER NOT NULL, can_upload INTEGER NOT NULL,
                    can_voice INTEGER NOT NULL, base_tier TEXT NOT NULL,
                    sort_order INTEGER NOT NULL DEFAULT 0);
                 INSERT INTO roles VALUES
                    ('verified','Verified','#4FC3F7',1,1,0,1,1,'verified',1),
                    ('admin','Admin','#E57373',4,1,1,1,1,'admin',4);",
            ).expect("rewind to old schema");
        }

        // 3. The upgrade. MUST NOT panic (the whole bug).
        let db = Storage::open(&path).expect("UPGRADE open must not panic");

        // 4. New columns exist + were applied correctly.
        let verified = db.role_def("verified");
        assert!(verified.can_image_share, "ALTER added can_image_share (DEFAULT 1)");
        assert!(verified.can_file_share);
        // base_tier 'verified' → backfilled from server_settings defaults.
        assert_eq!(verified.max_chars, 1000, "R4 backfill from base_tier");
        assert_eq!(verified.max_upload_mb, 25);
        assert_eq!(verified.max_uploads_kept, 20);
        // Seed (OR IGNORE) added the built-ins missing from the rewind.
        assert_eq!(db.role_def("mod").max_chars, 4000, "missing built-in seeded");
        assert!(db.list_roles().unwrap().iter().any(|r| r.id == "unverified"));
    }

    /// Ship homes increment 5: the ship-editing rank round-trips through
    /// upsert/list/role_def, on and back off (the ON CONFLICT path), and a
    /// payload without the field reads as no rank. Guards the appended
    /// positional SQL (?16, get(15)). A fresh database seeds it on Admin
    /// alone.
    ///
    /// Seen red 2026-10-05 with role_def's get(15) reading another column
    /// (get(14), max_uploads_kept, which is 4 for this role): "turned off,
    /// role_def reads it off".
    #[test]
    fn can_edit_ship_round_trips() {
        let db = fresh_db();
        let mut r = RoleDef::default();
        r.id = "shipwright".into();
        r.label = "Shipwright".into();
        r.built_in = false;
        r.base_tier = "verified".into();
        r.can_edit_ship = true;
        db.upsert_role(&r).expect("upsert");
        assert!(db.role_def("shipwright").can_edit_ship, "role_def reads the rank");
        let listed = db.list_roles().unwrap();
        assert!(listed.iter().find(|x| x.id == "shipwright").expect("in list").can_edit_ship, "list_roles reads the rank");

        r.can_edit_ship = false;
        db.upsert_role(&r).unwrap();
        assert!(!db.role_def("shipwright").can_edit_ship, "turned off, role_def reads it off");
        let listed = db.list_roles().unwrap();
        assert!(!listed.iter().find(|x| x.id == "shipwright").unwrap().can_edit_ship, "turned off, list_roles reads it off");

        for role in &listed {
            if role.built_in {
                assert_eq!(role.can_edit_ship, role.id == "admin", "a fresh server seeds the rank on Admin alone: {}", role.id);
            }
        }
        let old: RoleDef = serde_json::from_str(
            r##"{"id":"x","label":"X","color":"#000000","trust_level":1,"built_in":false,"can_stream":false,"can_upload":false,"can_voice":false,"base_tier":"verified","sort_order":1}"##,
        )
        .expect("a role without the field parses");
        assert!(!old.can_edit_ship, "a role sent without the field has no rank");
    }

    /// Ship homes increment 5, the upgrade path (the 2026-05-17 incident's
    /// lesson): a `roles` table from before the rank (the v0.1456 shape,
    /// fifteen columns) with its rows, reopened by this code, gains
    /// `can_edit_ship`, and only the built-in Admin role has it (a custom
    /// role does not). The rank is given to Admin once, with the column: an
    /// admin who turns it off finds it still off after a restart.
    ///
    /// Seen red 2026-10-05 with the guarded ALTER removed: "the upgrade
    /// opens: SqliteFailure(Error { code: Unknown, extended_code: 1 },
    /// Some(\"table roles has no column named can_edit_ship\"))" (the seed's
    /// INSERT names the column; the older
    /// `upgrade_from_pre_v0261_roles_schema_does_not_panic` failed the same
    /// way). The last assertion is there for an UPDATE that would run on
    /// every open instead of once, with the ALTER.
    #[test]
    fn an_existing_roles_table_gains_can_edit_ship_and_only_admin_has_it() {
        use rusqlite::Connection;
        let path = crate::test_temp::db("roles_ship_rank");
        drop(Storage::open(&path).expect("fresh open"));
        {
            let c = Connection::open(&path).expect("raw open");
            c.execute_batch(
                "DROP TABLE roles;
                 CREATE TABLE roles (
                    id TEXT PRIMARY KEY, label TEXT NOT NULL, color TEXT NOT NULL,
                    trust_level INTEGER NOT NULL, built_in INTEGER NOT NULL,
                    can_stream INTEGER NOT NULL, can_upload INTEGER NOT NULL,
                    can_voice INTEGER NOT NULL, base_tier TEXT NOT NULL,
                    sort_order INTEGER NOT NULL DEFAULT 0,
                    can_image_share INTEGER NOT NULL DEFAULT 1,
                    can_file_share  INTEGER NOT NULL DEFAULT 1,
                    max_chars        INTEGER NOT NULL DEFAULT 280,
                    max_upload_mb    INTEGER NOT NULL DEFAULT 5,
                    max_uploads_kept INTEGER NOT NULL DEFAULT 4);
                 INSERT INTO roles VALUES
                    ('unverified','Unverified','#9E9E9E',0,1,0,0,0,'unverified',0,1,1,280,5,4),
                    ('verified','Verified','#4FC3F7',1,1,0,1,1,'verified',1,1,1,1000,25,20),
                    ('donor','Donor','#FFD54F',2,1,0,1,1,'verified',2,1,1,1000,25,20),
                    ('mod','Moderator','#81C784',3,1,1,1,1,'mod',3,1,1,4000,100,100),
                    ('admin','Admin','#E57373',4,1,1,1,1,'admin',4,1,1,10000,500,500),
                    ('family','Family','#7E57C2',1,0,1,1,1,'verified',50,1,1,1000,25,20);",
            )
            .expect("rewind to the shape before the rank");
        }

        let db = Storage::open(&path).expect("the upgrade opens");
        let roles = db.list_roles().unwrap();
        assert_eq!(roles.len(), 6, "every role is kept");
        for r in &roles {
            assert_eq!(r.can_edit_ship, r.id == "admin", "after the upgrade only Admin has the rank: {}", r.id);
        }
        assert_eq!(db.role_def("family").max_chars, 1000, "a custom role keeps its own numbers");

        let mut admin = db.role_def("admin");
        admin.can_edit_ship = false;
        db.upsert_role(&admin).unwrap();
        drop(db);
        let db = Storage::open(&path).expect("restart");
        assert!(!db.role_def("admin").can_edit_ship, "an admin who turned the rank off for Admin finds it off after a restart");
    }

    /// Ship homes increment 5: who holds the ship-editing rank. The server's
    /// owner always does; a member through a role that has it (Admin, by
    /// default, or a custom role given it); a role without it, no role, or a
    /// role that was deleted, does not.
    ///
    /// Seen red 2026-10-05 with the owner's arm taken out of
    /// `has_ship_rank`: "the server's owner has the rank".
    #[test]
    fn the_ship_rank_is_the_owner_or_a_role_that_has_it() {
        let db = fresh_db();
        let mut wright = RoleDef::default();
        wright.id = "shipwright".into();
        wright.label = "Shipwright".into();
        wright.built_in = false;
        wright.can_edit_ship = true;
        db.upsert_role(&wright).unwrap();
        for (key, role) in [("0a", "owner"), ("0b", "admin"), ("0c", "verified"), ("0d", "shipwright")] {
            db.set_role(key, role).unwrap();
        }
        assert!(db.has_ship_rank("0a"), "the server's owner has the rank");
        assert!(db.has_ship_rank("0b"), "the built-in Admin role has it");
        assert!(!db.has_ship_rank("0c"), "Verified has not");
        assert!(db.has_ship_rank("0d"), "a custom role given it has it");
        assert!(!db.has_ship_rank("0e"), "nor has a member with no role");
        db.delete_role("shipwright").unwrap();
        assert!(!db.has_ship_rank("0d"), "a role that was deleted gives nothing");
    }

    /// B1, B2, B3 and B5 (2026-10-11), the upgrade path (BUG-046): a `roles` table from before the
    /// upload limits (the sixteen columns up to `can_edit_ship`) with its rows, reopened by this
    /// code, gains `max_media_file_mb` (0), `max_private_file_mb` (6), and the day's, the hour's
    /// and the private-held allowances by tier: Admin none, Moderator 4096 MB / 200 / 120 MB,
    /// Verified and anything on its tier 1024 MB / 60 / 60 MB, the rest 100 MB / 20 / 30 MB. An
    /// admin's later change survives a restart. A fresh database seeds the same numbers. The
    /// limits round-trip through upsert, and a number out of range is held to it when saved.
    ///
    /// Seen red 2026-10-11 with the guarded ALTER removed: "the upgrade opens:
    /// SqliteFailure(Error { code: Unknown, extended_code: 1 }, Some(\"table roles has no column
    /// named max_upload_mb_per_day\"))" (the seed names the column).
    #[test]
    fn an_existing_roles_table_gains_the_upload_limits() {
        use rusqlite::Connection;
        let path = crate::test_temp::db("roles_upload_limits");
        drop(Storage::open(&path).expect("fresh open"));
        let fresh = Storage::open(&path).unwrap();
        let tiers = |db: &Storage| -> Vec<(String, i64, i64, i64, i64, i64)> {
            db.list_roles()
                .unwrap()
                .into_iter()
                .map(|r| (r.id, r.max_media_file_mb, r.max_private_file_mb, r.max_upload_mb_per_day, r.max_uploads_per_hour, r.max_private_held_mb))
                .collect()
        };
        let expected = |custom: bool| {
            let mut v = vec![
                ("unverified".to_string(), 0, 6, 100, 20, 30),
                ("verified".to_string(), 0, 6, 1024, 60, 60),
                ("donor".to_string(), 0, 6, 1024, 60, 60),
                ("mod".to_string(), 0, 6, 4096, 200, 120),
                ("admin".to_string(), 0, 6, 0, 0, 0),
            ];
            if custom {
                v.push(("family".to_string(), 0, 6, 1024, 60, 60));
            }
            v
        };
        assert_eq!(tiers(&fresh), expected(false), "a fresh database seeds the numbers");
        drop(fresh);
        {
            let c = Connection::open(&path).expect("raw open");
            c.execute_batch(
                "DROP TABLE roles;
                 CREATE TABLE roles (
                    id TEXT PRIMARY KEY, label TEXT NOT NULL, color TEXT NOT NULL,
                    trust_level INTEGER NOT NULL, built_in INTEGER NOT NULL,
                    can_stream INTEGER NOT NULL, can_upload INTEGER NOT NULL,
                    can_voice INTEGER NOT NULL, base_tier TEXT NOT NULL,
                    sort_order INTEGER NOT NULL DEFAULT 0,
                    can_image_share INTEGER NOT NULL DEFAULT 1,
                    can_file_share  INTEGER NOT NULL DEFAULT 1,
                    max_chars        INTEGER NOT NULL DEFAULT 280,
                    max_upload_mb    INTEGER NOT NULL DEFAULT 5,
                    max_uploads_kept INTEGER NOT NULL DEFAULT 4,
                    can_edit_ship    INTEGER NOT NULL DEFAULT 0);
                 INSERT INTO roles VALUES
                    ('unverified','Unverified','#9E9E9E',0,1,0,0,0,'unverified',0,1,1,280,5,4,0),
                    ('verified','Verified','#4FC3F7',1,1,0,1,1,'verified',1,1,1,1000,25,20,0),
                    ('donor','Donor','#FFD54F',2,1,0,1,1,'verified',2,1,1,1000,25,20,0),
                    ('mod','Moderator','#81C784',3,1,1,1,1,'mod',3,1,1,4000,100,100,0),
                    ('admin','Admin','#E57373',4,1,1,1,1,'admin',4,1,1,10000,500,500,1),
                    ('family','Family','#7E57C2',1,0,1,1,1,'verified',50,1,1,1000,25,20,0);",
            )
            .expect("rewind to the shape before the upload limits");
        }
        let db = Storage::open(&path).expect("the upgrade opens");
        assert_eq!(tiers(&db), expected(true), "every role gains its tier's limits");

        let mut verified = db.role_def("verified");
        verified.max_upload_mb_per_day = 5;
        verified.max_media_file_mb = 2;
        db.upsert_role(&verified).unwrap();
        drop(db);
        let db = Storage::open(&path).expect("restart");
        let v = db.role_def("verified");
        assert_eq!((v.max_upload_mb_per_day, v.max_media_file_mb), (5, 2), "an admin's change survives a restart");

        let mut wild = v.clone();
        wild.max_media_file_mb = -3;
        wild.max_private_file_mb = 0;
        wild.max_upload_mb_per_day = i64::MAX;
        wild.max_uploads_per_hour = -1;
        wild.max_private_held_mb = -9;
        db.upsert_role(&wild).unwrap();
        let held = db.role_def("verified");
        assert_eq!(
            (held.max_media_file_mb, held.max_private_file_mb, held.max_upload_mb_per_day, held.max_uploads_per_hour, held.max_private_held_mb),
            (0, 1, UPLOAD_MB_PER_DAY_MAX, 0, 0),
            "out of range, held to the range"
        );
        let old: RoleDef = serde_json::from_str(
            r##"{"id":"x","label":"X","color":"#000000","trust_level":1,"built_in":false,"can_stream":false,"can_upload":true,"can_voice":false,"base_tier":"verified","sort_order":1}"##,
        )
        .unwrap();
        // The frame itself carries "not sent"; the save gives a new role the careful defaults
        // (`unsent_limits_kept`, 2026-10-11).
        db.upsert_role(&old).unwrap();
        let saved = db.role_def("x");
        assert_eq!((saved.max_private_file_mb, saved.max_upload_mb_per_day, saved.max_uploads_per_hour, saved.max_private_held_mb), (6, 100, 20, 30), "a role sent without them gets the careful defaults");
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

    /// A ROLE SAVED BY AN OLDER APP KEEPS ITS UPLOAD LIMITS: a v0.1484.0 app saves a role without
    /// the five limits it does not know, and the save keeps the stored values instead of resetting
    /// them to defaults (Verified's 1024 MB a day would have become 0, "no limit", after the
    /// range clamp). A new role saved the same way gets the defaults. (Persisted-format review,
    /// 2026-10-11.)
    ///
    /// Seen red 2026-10-11 with the keep step left out of `upsert_role`: "an older app's save
    /// keeps Verified's limits / left: (0, 1, 0, 0, 0) / right: (0, 6, 1024, 60, 60)".
    #[test]
    fn a_role_saved_by_an_older_app_keeps_its_upload_limits() {
        let db = fresh_db();
        let verified = db.list_roles().unwrap().into_iter().find(|r| r.id == "verified").expect("seeded");
        let limits = |r: &RoleDef| (r.max_media_file_mb, r.max_private_file_mb, r.max_upload_mb_per_day, r.max_uploads_per_hour, r.max_private_held_mb);
        let before = limits(&verified);
        assert_eq!(before, (0, 6, 1024, 60, 60), "test setup: the seeded Verified limits");

        // The frame an older app sends: the role as it knows it, without the five fields.
        let mut old = serde_json::to_value(&verified).unwrap();
        for k in ["max_media_file_mb", "max_private_file_mb", "max_upload_mb_per_day", "max_uploads_per_hour", "max_private_held_mb"] {
            old.as_object_mut().unwrap().remove(k);
        }
        old["label"] = serde_json::json!("Verified (renamed)");
        let sent: RoleDef = serde_json::from_value(old.clone()).unwrap();
        db.upsert_role(&sent).unwrap();
        let after = db.list_roles().unwrap().into_iter().find(|r| r.id == "verified").unwrap();
        assert_eq!(after.label, "Verified (renamed)", "the save itself went through");
        assert_eq!(limits(&after), before, "an older app's save keeps Verified's limits");

        old["id"] = serde_json::json!("helper");
        let fresh: RoleDef = serde_json::from_value(old).unwrap();
        db.upsert_role(&fresh).unwrap();
        let helper = db.list_roles().unwrap().into_iter().find(|r| r.id == "helper").expect("the new role");
        assert_eq!(limits(&helper), limits(&RoleDef::default()), "a new role saved that way gets the defaults");
    }

}
