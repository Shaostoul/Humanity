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
    /// Max upload size in megabytes. Applies to both images and files.
    pub max_upload_mb: i64,
    /// Whether voice channels can be created/used (server-wide).
    pub voice_channels_enabled: bool,
    /// Whether video streaming is enabled (server-wide). Default OFF
    /// because it's bandwidth-heavy.
    pub video_streaming_enabled: bool,
    /// Comma-separated list of allowed file extensions, lowercase, no
    /// leading dot. e.g. "png,jpg,pdf,txt". Empty string = no
    /// restriction (any extension allowed).
    pub allowed_file_extensions: String,
    /// Last update unix-millis. 0 = never updated since creation.
    pub updated_at: i64,
    /// Public key of the admin who last touched it. Empty = never.
    pub updated_by: String,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            max_chars_unverified: 280,
            max_chars_verified: 1000,
            max_chars_mod: 4000,
            max_chars_admin: 10000,
            image_sharing_enabled: true,
            file_sharing_enabled: true,
            max_upload_mb: 25,
            voice_channels_enabled: true,
            video_streaming_enabled: false,
            allowed_file_extensions: "png,jpg,jpeg,gif,webp,pdf,txt,md".to_string(),
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
                        allowed_file_extensions, updated_at, COALESCE(updated_by, '')
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
                    max_chars_unverified    = ?1,
                    max_chars_verified      = ?2,
                    max_chars_mod           = ?3,
                    max_chars_admin         = ?4,
                    image_sharing_enabled   = ?5,
                    file_sharing_enabled    = ?6,
                    max_upload_mb           = ?7,
                    voice_channels_enabled  = ?8,
                    video_streaming_enabled = ?9,
                    allowed_file_extensions = ?10,
                    updated_at              = ?11,
                    updated_by              = ?12
                 WHERE id = 1",
                params![
                    s.max_chars_unverified,
                    s.max_chars_verified,
                    s.max_chars_mod,
                    s.max_chars_admin,
                    s.image_sharing_enabled as i32,
                    s.file_sharing_enabled as i32,
                    s.max_upload_mb,
                    s.voice_channels_enabled as i32,
                    s.video_streaming_enabled as i32,
                    s.allowed_file_extensions,
                    now,
                    updated_by,
                ],
            )?;
            Ok(rows > 0)
        })
    }
}
