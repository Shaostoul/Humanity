//! Server settings singleton (v0.200.0).
//!
//! One row per server (id = 1, enforced by CHECK constraint). Holds
//! operator-tunable policies — message length limits per role tier,
//! file/image/voice/streaming toggles, max upload size, allowed
//! extensions. Exposed via the admin UI in Server Settings → Admin
//! section. See `docs/design/storage-architecture.md` for the wider
//! storage model.

use super::Storage;
use rusqlite::params;
use serde::{Deserialize, Serialize};

/// Server-wide policy settings. Mirrors the `server_settings` SQLite
/// row exactly. Both sides of the WS protocol (relay + native client)
/// use this shape via serde.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerSettings {
    /// Max characters in a chat message for unverified users.
    pub max_chars_unverified: i64,
    /// Max characters for verified users.
    pub max_chars_verified: i64,
    /// Max characters for moderators.
    pub max_chars_mod: i64,
    /// Max characters for admins.
    pub max_chars_admin: i64,
    /// Whether image attachments are allowed (server-wide).
    pub image_sharing_enabled: bool,
    /// Whether file attachments are allowed (server-wide).
    pub file_sharing_enabled: bool,
    /// LEGACY (v0.200): single max upload MB. Kept for backward
    /// compatibility with old clients that haven't been updated. New
    /// code should use the per-role variants below. v0.201+ keeps this
    /// in sync with `max_upload_mb_unverified` so old clients see at
    /// least the most-conservative limit.
    pub max_upload_mb: i64,
    /// Max upload size (MB) for unverified users. Default 5.
    #[serde(default = "default_upload_unverified")]
    pub max_upload_mb_unverified: i64,
    /// Max upload size (MB) for verified users. Default 25.
    #[serde(default = "default_upload_verified")]
    pub max_upload_mb_verified: i64,
    /// Max upload size (MB) for moderators. Default 100.
    #[serde(default = "default_upload_mod")]
    pub max_upload_mb_mod: i64,
    /// Max upload size (MB) for admins. Default 500.
    #[serde(default = "default_upload_admin")]
    pub max_upload_mb_admin: i64,
    /// Whether voice channels can be created/used (server-wide).
    pub voice_channels_enabled: bool,
    /// Whether video streaming is enabled (server-wide). Default OFF
    /// because it's bandwidth-heavy.
    pub video_streaming_enabled: bool,
    /// Comma-separated list of allowed file extensions, lowercase, no
    /// leading dot. e.g. "png,jpg,pdf,txt". Empty string = no
    /// restriction (any extension allowed).
    pub allowed_file_extensions: String,
    /// LEGACY (v0.237): single per-user FIFO retention count. Kept so
    /// v0.237 clients keep working; v0.238+ uses the per-role variants
    /// below and keeps this synced with the unverified value (most
    /// conservative), mirroring how `max_upload_mb` shadows its
    /// per-role split.
    #[serde(default = "default_max_uploads_per_user")]
    pub max_uploads_per_user: i64,
    /// How many uploads to KEEP per user (FIFO) by role. When a user's
    /// upload count exceeds their role's limit, the oldest are deleted
    /// from disk. Trust ladder — more trusted users keep more history.
    /// v0.238 — operator: "expand the sensible settings to include per
    /// ranking such as unverified, verified, mod, admin."
    #[serde(default = "default_uploads_kept_unverified")]
    pub max_uploads_per_user_unverified: i64,
    #[serde(default = "default_uploads_kept_verified")]
    pub max_uploads_per_user_verified: i64,
    #[serde(default = "default_uploads_kept_mod")]
    pub max_uploads_per_user_mod: i64,
    #[serde(default = "default_uploads_kept_admin")]
    pub max_uploads_per_user_admin: i64,
    /// Server-wide total upload disk cap in MB. New uploads are rejected
    /// once the uploads directory would exceed this. NOT per-role — it's
    /// a physical disk constraint, one number for the whole relay.
    /// Default 500. v0.237 — was a hardcoded `500 * 1024 * 1024`.
    #[serde(default = "default_max_total_upload_mb")]
    pub max_total_upload_mb: i64,
    /// PQ migration Increment 3: when true, the relay REJECTS a chat
    /// message from an account that has a Dilithium3 key on file unless
    /// it carries a valid `pq_signature` (quantum-forgery resistance).
    /// Accounts with NO PQ key on file (old/incapable clients) are still
    /// accepted on Ed25519 — they're never locked out, they just aren't
    /// PQ-protected yet. Default FALSE: the operator flips this on once
    /// the `pq_dualsign` telemetry shows members have all reconnected on
    /// a PQ-capable client (v0.251+). Fully reversible.
    #[serde(default)]
    pub require_pq_signatures: bool,
    /// SOFT gate for the future P2P content-distribution feature
    /// (operator-uploaded 3D models seeded via BitTorrent). When OFF
    /// the relay must not generate/serve torrents or magnet links.
    /// Default FALSE — the feature isn't built yet; this is the
    /// plumbing + a documented no-op gate so the Services panel has a
    /// real switch from day one. The matching OS daemon
    /// (transmission-daemon) is controlled separately via the
    /// service-control bridge. v0.262.16.
    #[serde(default)]
    pub p2p_distribution_enabled: bool,
    /// Last update unix-millis. 0 = never updated since creation.
    pub updated_at: i64,
    /// Public key of the admin who last touched it. Empty = never.
    pub updated_by: String,
}

// Per-role upload defaults (v0.201). Trust ladder — more trusted users
// get more bandwidth. Operators can tune via Server Settings → Admin.
fn default_upload_unverified() -> i64 { 5 }
fn default_upload_verified() -> i64 { 25 }
fn default_upload_mod() -> i64 { 100 }
fn default_upload_admin() -> i64 { 500 }
fn default_max_uploads_per_user() -> i64 { 4 }
fn default_max_total_upload_mb() -> i64 { 500 }
// Per-role FIFO retention defaults (v0.238). Trust ladder — unverified
// users keep the historical 4; trusted tiers keep more.
fn default_uploads_kept_unverified() -> i64 { 4 }
fn default_uploads_kept_verified() -> i64 { 20 }
fn default_uploads_kept_mod() -> i64 { 100 }
fn default_uploads_kept_admin() -> i64 { 500 }

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            max_chars_unverified: 280,
            max_chars_verified: 1000,
            max_chars_mod: 4000,
            max_chars_admin: 10000,
            image_sharing_enabled: true,
            file_sharing_enabled: true,
            max_upload_mb: 25, // legacy mirror of unverified value
            max_upload_mb_unverified: default_upload_unverified(),
            max_upload_mb_verified: default_upload_verified(),
            max_upload_mb_mod: default_upload_mod(),
            max_upload_mb_admin: default_upload_admin(),
            voice_channels_enabled: true,
            video_streaming_enabled: false,
            allowed_file_extensions: "png,jpg,jpeg,gif,webp,pdf,txt,md".to_string(),
            max_uploads_per_user: default_max_uploads_per_user(),
            max_uploads_per_user_unverified: default_uploads_kept_unverified(),
            max_uploads_per_user_verified: default_uploads_kept_verified(),
            max_uploads_per_user_mod: default_uploads_kept_mod(),
            max_uploads_per_user_admin: default_uploads_kept_admin(),
            max_total_upload_mb: default_max_total_upload_mb(),
            require_pq_signatures: false, // operator opts in when adoption is confirmed
            p2p_distribution_enabled: false, // feature unbuilt; off until operator + feature ready
            updated_at: 0,
            updated_by: String::new(),
        }
    }
}

impl ServerSettings {
    /// Lookup the max-chars limit for a given role string.
    /// Falls back to `max_chars_unverified` for any unknown role.
    pub fn max_chars_for_role(&self, role: &str) -> i64 {
        match role {
            "admin" | "owner" => self.max_chars_admin,
            "mod" => self.max_chars_mod,
            "verified" => self.max_chars_verified,
            _ => self.max_chars_unverified,
        }
    }

    /// Lookup the max-upload-MB limit for a given role string (v0.201).
    /// Falls back to `max_upload_mb_unverified` for any unknown role.
    pub fn max_upload_mb_for_role(&self, role: &str) -> i64 {
        match role {
            "admin" | "owner" => self.max_upload_mb_admin,
            "mod" => self.max_upload_mb_mod,
            "verified" => self.max_upload_mb_verified,
            _ => self.max_upload_mb_unverified,
        }
    }

    /// Lookup the per-user FIFO retention count for a given role string
    /// (v0.238). Falls back to the unverified value for unknown roles.
    pub fn max_uploads_per_user_for_role(&self, role: &str) -> i64 {
        match role {
            "admin" | "owner" => self.max_uploads_per_user_admin,
            "mod" => self.max_uploads_per_user_mod,
            "verified" => self.max_uploads_per_user_verified,
            _ => self.max_uploads_per_user_unverified,
        }
    }
}

impl Storage {
    /// Read the singleton server_settings row. Returns Default if the
    /// row is missing for some reason (defensive — the migration
    /// inserts the row at startup).
    pub fn get_server_settings(&self) -> Result<ServerSettings, rusqlite::Error> {
        self.with_conn(|conn| {
            match conn.query_row(
                "SELECT max_chars_unverified, max_chars_verified, max_chars_mod, max_chars_admin,
                        image_sharing_enabled, file_sharing_enabled, max_upload_mb,
                        voice_channels_enabled, video_streaming_enabled,
                        allowed_file_extensions, updated_at, COALESCE(updated_by, ''),
                        max_upload_mb_unverified, max_upload_mb_verified,
                        max_upload_mb_mod, max_upload_mb_admin,
                        max_uploads_per_user, max_total_upload_mb,
                        max_uploads_per_user_unverified, max_uploads_per_user_verified,
                        max_uploads_per_user_mod, max_uploads_per_user_admin,
                        require_pq_signatures, p2p_distribution_enabled
                 FROM server_settings WHERE id = 1",
                [],
                |row| {
                    let img: i32 = row.get(4)?;
                    let file: i32 = row.get(5)?;
                    let voice: i32 = row.get(7)?;
                    let video: i32 = row.get(8)?;
                    let req_pq: i32 = row.get(22)?;
                    let p2p: i32 = row.get(23)?;
                    Ok(ServerSettings {
                        max_chars_unverified: row.get(0)?,
                        max_chars_verified: row.get(1)?,
                        max_chars_mod: row.get(2)?,
                        max_chars_admin: row.get(3)?,
                        image_sharing_enabled: img != 0,
                        file_sharing_enabled: file != 0,
                        max_upload_mb: row.get(6)?,
                        voice_channels_enabled: voice != 0,
                        video_streaming_enabled: video != 0,
                        allowed_file_extensions: row.get(9)?,
                        updated_at: row.get(10)?,
                        updated_by: row.get(11)?,
                        max_upload_mb_unverified: row.get(12)?,
                        max_upload_mb_verified: row.get(13)?,
                        max_upload_mb_mod: row.get(14)?,
                        max_upload_mb_admin: row.get(15)?,
                        max_uploads_per_user: row.get(16)?,
                        max_total_upload_mb: row.get(17)?,
                        max_uploads_per_user_unverified: row.get(18)?,
                        max_uploads_per_user_verified: row.get(19)?,
                        max_uploads_per_user_mod: row.get(20)?,
                        max_uploads_per_user_admin: row.get(21)?,
                        require_pq_signatures: req_pq != 0,
                        p2p_distribution_enabled: p2p != 0,
                    })
                },
            ) {
                Ok(s) => Ok(s),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(ServerSettings::default()),
                Err(e) => Err(e),
            }
        })
    }

    /// Persist the server_settings row, stamping updated_at + updated_by.
    /// Returns true on success. Caller (WS handler) is responsible for
    /// admin-permission validation BEFORE calling this.
    /// v0.201: also keeps the legacy `max_upload_mb` column synced with
    /// `max_upload_mb_unverified` so old clients still see a useful value.
    pub fn set_server_settings(
        &self,
        s: &ServerSettings,
        updated_by: &str,
    ) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            let rows = conn.execute(
                "UPDATE server_settings SET
                    max_chars_unverified     = ?1,
                    max_chars_verified       = ?2,
                    max_chars_mod            = ?3,
                    max_chars_admin          = ?4,
                    image_sharing_enabled    = ?5,
                    file_sharing_enabled     = ?6,
                    max_upload_mb            = ?7,
                    voice_channels_enabled   = ?8,
                    video_streaming_enabled  = ?9,
                    allowed_file_extensions  = ?10,
                    max_upload_mb_unverified = ?11,
                    max_upload_mb_verified   = ?12,
                    max_upload_mb_mod        = ?13,
                    max_upload_mb_admin      = ?14,
                    max_uploads_per_user     = ?17,
                    max_total_upload_mb      = ?18,
                    max_uploads_per_user_unverified = ?19,
                    max_uploads_per_user_verified   = ?20,
                    max_uploads_per_user_mod        = ?21,
                    max_uploads_per_user_admin      = ?22,
                    require_pq_signatures           = ?23,
                    p2p_distribution_enabled        = ?24,
                    updated_at               = ?15,
                    updated_by               = ?16
                 WHERE id = 1",
                params![
                    s.max_chars_unverified,
                    s.max_chars_verified,
                    s.max_chars_mod,
                    s.max_chars_admin,
                    s.image_sharing_enabled as i32,
                    s.file_sharing_enabled as i32,
                    // Legacy single column tracks unverified (most conservative).
                    s.max_upload_mb_unverified,
                    s.voice_channels_enabled as i32,
                    s.video_streaming_enabled as i32,
                    s.allowed_file_extensions,
                    s.max_upload_mb_unverified,
                    s.max_upload_mb_verified,
                    s.max_upload_mb_mod,
                    s.max_upload_mb_admin,
                    now,
                    updated_by,
                    // Legacy single col tracks unverified (most conservative).
                    s.max_uploads_per_user_unverified,
                    s.max_total_upload_mb,
                    s.max_uploads_per_user_unverified,
                    s.max_uploads_per_user_verified,
                    s.max_uploads_per_user_mod,
                    s.max_uploads_per_user_admin,
                    s.require_pq_signatures as i32,
                    s.p2p_distribution_enabled as i32,
                ],
            )?;
            Ok(rows > 0)
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
        let path = std::env::temp_dir().join(format!("hum_srvset_{pid}_{nanos}.db"));
        Storage::open(&path).expect("open test db")
    }

    /// PQ Inc 3: require_pq_signatures must default OFF and round-trip
    /// through the positional set/get SQL. Guards against an ?N column
    /// index mistake silently corrupting the toggle (which would either
    /// never enforce, or — worse — enforce unexpectedly and lock users
    /// out). Also re-checks an existing bool + an int so a shifted index
    /// is caught broadly.
    #[test]
    fn require_pq_signatures_roundtrips_and_defaults_off() {
        let db = fresh_db();
        let s = db.get_server_settings().expect("get");
        assert!(!s.require_pq_signatures, "MUST default OFF (no surprise lockout)");
        assert!(s.image_sharing_enabled, "sanity: existing default intact");

        let mut updated = s.clone();
        updated.require_pq_signatures = true;
        updated.video_streaming_enabled = true;       // another bool
        updated.max_total_upload_mb = 1234;           // an int, same row
        assert!(db.set_server_settings(&updated, "admin_key").expect("set"));

        let got = db.get_server_settings().expect("get2");
        assert!(got.require_pq_signatures, "toggle must persist ON");
        assert!(got.video_streaming_enabled);
        assert_eq!(got.max_total_upload_mb, 1234, "no positional-index bleed");
        assert_eq!(got.updated_by, "admin_key");

        // Reversible.
        let mut off = got.clone();
        off.require_pq_signatures = false;
        assert!(db.set_server_settings(&off, "admin_key").expect("set3"));
        assert!(!db.get_server_settings().expect("get3").require_pq_signatures);
    }

    /// Server→Services (v0.262.16): p2p_distribution_enabled must
    /// default OFF (the feature is unbuilt — it must never silently be
    /// "on") and round-trip cleanly through the positional set/get SQL.
    #[test]
    fn p2p_distribution_enabled_roundtrips_and_defaults_off() {
        let db = fresh_db();
        let s = db.get_server_settings().expect("get");
        assert!(
            !s.p2p_distribution_enabled,
            "MUST default OFF — feature is unbuilt, never silently on"
        );
        let mut on = s.clone();
        on.p2p_distribution_enabled = true;
        on.require_pq_signatures = true; // adjacent bool — catch index bleed
        assert!(db.set_server_settings(&on, "admin_key").expect("set"));
        let got = db.get_server_settings().expect("get2");
        assert!(got.p2p_distribution_enabled, "toggle must persist ON");
        assert!(got.require_pq_signatures, "no positional-index bleed");
        let mut off = got.clone();
        off.p2p_distribution_enabled = false;
        assert!(db.set_server_settings(&off, "admin_key").expect("set3"));
        assert!(!db.get_server_settings().expect("get3").p2p_distribution_enabled);
    }

    /// Incident-class regression (2026-05-17 lesson): a relay whose DB
    /// predates the p2p_distribution_enabled column must upgrade WITHOUT
    /// panicking, default the new column OFF, and PRESERVE the
    /// operator's existing tuned values. Rewinds the live schema by
    /// dropping the column on a raw connection, then reopens Storage so
    /// the guarded ALTER runs the real migration path.
    #[test]
    fn upgrade_from_pre_p2p_server_settings_schema_does_not_panic() {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_srvset_upg_{pid}_{nanos}.db"));

        // 1. Fresh DB, then simulate an operator who tuned settings
        //    BEFORE this migration existed.
        {
            let db = Storage::open(&path).expect("open v1");
            let mut s = db.get_server_settings().expect("get v1");
            s.require_pq_signatures = true; // a bool the operator set
            s.max_total_upload_mb = 4321;   // an int the operator set
            assert!(db.set_server_settings(&s, "op_key").expect("set v1"));
        }
        // 2. Rewind: drop the new column so the file looks pre-v0.262.16.
        {
            let conn = rusqlite::Connection::open(&path).expect("raw open");
            conn.execute_batch(
                "ALTER TABLE server_settings DROP COLUMN p2p_distribution_enabled;",
            )
            .expect("drop column (SQLite >= 3.35)");
            assert!(
                conn.prepare("SELECT p2p_distribution_enabled FROM server_settings LIMIT 0")
                    .is_err(),
                "column must really be gone — test premise"
            );
        }
        // 3. Reopen Storage — the guarded ALTER must run the migration.
        let db = Storage::open(&path).expect("reopen MUST NOT panic (incident regression)");
        let got = db
            .get_server_settings()
            .expect("get_server_settings after upgrade MUST NOT error");
        assert!(
            !got.p2p_distribution_enabled,
            "migration backfill MUST default the new column OFF"
        );
        assert!(
            got.require_pq_signatures,
            "operator's pre-migration bool MUST be preserved"
        );
        assert_eq!(
            got.max_total_upload_mb, 4321,
            "operator's pre-migration int MUST be preserved (non-destructive upgrade)"
        );
        let _ = std::fs::remove_file(&path);
    }
}
