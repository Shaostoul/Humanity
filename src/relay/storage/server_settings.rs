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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// How many uploads to KEEP per user (FIFO). When a user's upload
    /// count exceeds this, the oldest are deleted from disk. Default 4.
    /// v0.237 — was a hardcoded `4` in storage/uploads.rs.
    #[serde(default = "default_max_uploads_per_user")]
    pub max_uploads_per_user: i64,
    /// Server-wide total upload disk cap in MB. New uploads are rejected
    /// once the uploads directory would exceed this. Default 500.
    /// v0.237 — was a hardcoded `500 * 1024 * 1024` in relay/api.rs.
    #[serde(default = "default_max_total_upload_mb")]
    pub max_total_upload_mb: i64,
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
            max_total_upload_mb: default_max_total_upload_mb(),
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
                        max_uploads_per_user, max_total_upload_mb
                 FROM server_settings WHERE id = 1",
                [],
                |row| {
                    let img: i32 = row.get(4)?;
                    let file: i32 = row.get(5)?;
                    let voice: i32 = row.get(7)?;
                    let video: i32 = row.get(8)?;
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
                    s.max_uploads_per_user,
                    s.max_total_upload_mb,
                ],
            )?;
            Ok(rows > 0)
        })
    }
}
