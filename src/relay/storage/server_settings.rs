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
    /// Operator-set description of this server, shown in the launcher's
    /// server-detail pane (v0.478). Plain text, empty by default. Editable by
    /// admins/owners via the same admin-gated server_settings_update path.
    #[serde(default)]
    pub server_description: String,
    /// Operator-set display name of this server (v0.480). Empty = fall back to
    /// the boot-time server-config.json / SERVER_NAME env default. Editable by
    /// admins/owners via the same admin-gated server_settings_update path.
    #[serde(default)]
    pub server_name: String,
    /// Guaranteed local-only room (v0.1132). While ON (the default) the
    /// relay keeps a #local channel seeded and REFUSES to federate any
    /// channel flagged local_only, so members always have a room that
    /// provably never leaves this server. Turning it off stops the seeding
    /// and lifts the refusal; the room (if present) becomes an ordinary
    /// channel an admin can federate or delete.
    #[serde(default = "default_local_channel_enabled")]
    pub local_channel_enabled: bool,
    /// Days a sealed DM envelope sits in the dm_mailbox before the relay
    /// expires it (sealed-sender store-and-forward, 2026-08-23). Clients
    /// keep long-term DM history locally; the server holds only this
    /// delivery window, so a subpoena/breach yields at most this many
    /// days of pseudonymous, sender-less ciphertext blobs. Minimum 1.
    #[serde(default = "default_dm_mailbox_ttl_days")]
    pub dm_mailbox_ttl_days: i64,
    /// Days to keep public channel messages before auto-expiring them
    /// (privacy maximization, 2026-08-24). 0 = keep forever (the default,
    /// preserving current behavior). Pinned messages are always kept.
    #[serde(default)]
    pub message_retention_days: i64,
    /// Days this relay remembers that an account was erased here, as a one-way fingerprint
    /// of its key and the day (storage/erased_accounts.rs, BUG-135, 2026-10-04), so the
    /// account's other devices are not signed up again by themselves. Older entries are
    /// deleted. Minimum 1, default 30.
    #[serde(default = "default_erased_accounts_ttl_days")]
    pub erased_accounts_ttl_days: i64,
    /// The most erased accounts remembered at once; when full, the oldest go first, so a
    /// flood of erases cannot grow the table without bound. Default 100,000.
    #[serde(default = "default_erased_accounts_cap")]
    pub erased_accounts_cap: i64,
    /// How fast the shared world's clock runs, game seconds per real second
    /// (operator, 2026-10-04: "let's do 72x but, make sure there's admin
    /// tools for me to adjust it from inside the app"). 72 by default, the
    /// Simplified speed: a day in 20 real minutes. Every game in the shared
    /// world follows it (`game_time_sync` carries it); an admin changes it
    /// from Server Settings > ADMIN > Shared world clock, live, no restart.
    #[serde(default = "default_world_time_scale")]
    pub world_time_scale: f64,
    /// Where the shared world's stores stand (the operator, 2026-10-04: "For the sake of
    /// simplicity during early development we'll say the fleet has unlimited of everything
    /// and just track what they player uses and contributes."): "unlimited", the default, the
    /// fleet never runs out and nobody misses a meal; or "stocked", the realistic mode built
    /// in ship homes increment 3, stores holding a stock the crew and players eat down and the
    /// ship's farms fill, an empty store a missed meal. Either way each player's fleet ledger
    /// keeps what they used and gave (storage/fleet_ledger.rs). An admin changes it from Server
    /// Settings > ADMIN > Fleet supply, live, no restart; a change never touches a ledger.
    #[serde(default = "default_fleet_supply_mode")]
    pub fleet_supply_mode: String,
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
fn default_local_channel_enabled() -> bool { true }
fn default_dm_mailbox_ttl_days() -> i64 { 30 }
fn default_erased_accounts_ttl_days() -> i64 { 30 }
fn default_erased_accounts_cap() -> i64 { 100_000 }

/// The ranges the two erased-accounts settings are held to (the relay clamps an update to
/// them, the Server Settings page offers exactly them): at least a day and one entry; at most
/// a year, the mailbox's ceiling, and a million entries (about 100 MB of fingerprints).
pub const ERASED_ACCOUNTS_TTL_DAYS_RANGE: (i64, i64) = (1, 365);
pub const ERASED_ACCOUNTS_CAP_RANGE: (i64, i64) = (1, 1_000_000);

/// The shared world's clock speed on a new server: the game's Simplified
/// speed (`systems::time::SIMPLIFIED_TIME_SPEED`, 72), one number for both.
/// The schema's `DEFAULT 72` (storage/mod.rs) is checked against it by
/// `world_time_scale_defaults_to_simplified_and_round_trips`.
pub fn default_world_time_scale() -> f64 {
    f64::from(crate::systems::time::SIMPLIFIED_TIME_SPEED)
}

/// The fleet's supply on a new server, and on one whose stored value is not a mode this code
/// knows: "unlimited" (the operator's early-development decision of 2026-10-04). The schema's
/// `DEFAULT 'unlimited'` (storage/mod.rs) is checked against it by
/// `fleet_supply_mode_defaults_to_unlimited_and_round_trips`.
pub const DEFAULT_FLEET_SUPPLY_MODE: &str = "unlimited";

fn default_fleet_supply_mode() -> String {
    DEFAULT_FLEET_SUPPLY_MODE.to_string()
}

/// A fleet supply mode as it is stored: "unlimited" or "stocked", whatever the case or
/// spacing it came in, or None for anything else (a bad message changes nothing).
pub fn fleet_supply_mode_of(v: &str) -> Option<&'static str> {
    match v.trim().to_ascii_lowercase().as_str() {
        "unlimited" => Some("unlimited"),
        "stocked" => Some("stocked"),
        _ => None,
    }
}

/// A world clock speed from an admin, inside the range a player's own Time
/// setting has (`systems::time::MIN_TIME_SPEED..=MAX_TIME_SPEED`, 1 to
/// 1,000). None for a value that is not a number at all, so a bad message
/// leaves the clock as it was rather than stopping or racing it.
pub fn clamp_world_time_scale(v: f64) -> Option<f64> {
    use crate::systems::time::{MAX_TIME_SPEED, MIN_TIME_SPEED};
    v.is_finite().then(|| v.clamp(f64::from(MIN_TIME_SPEED), f64::from(MAX_TIME_SPEED)))
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
            server_description: String::new(),
            server_name: String::new(),
            local_channel_enabled: default_local_channel_enabled(),
            dm_mailbox_ttl_days: default_dm_mailbox_ttl_days(),
            message_retention_days: 0,
            erased_accounts_ttl_days: default_erased_accounts_ttl_days(),
            erased_accounts_cap: default_erased_accounts_cap(),
            world_time_scale: default_world_time_scale(),
            fleet_supply_mode: default_fleet_supply_mode(),
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
                        require_pq_signatures, p2p_distribution_enabled,
                        COALESCE(server_description, ''), COALESCE(server_name, ''),
                        COALESCE(local_channel_enabled, 1),
                        COALESCE(dm_mailbox_ttl_days, 30),
                        COALESCE(message_retention_days, 0),
                        COALESCE(erased_accounts_ttl_days, 30),
                        COALESCE(erased_accounts_cap, 100000),
                        world_time_scale,
                        fleet_supply_mode
                 FROM server_settings WHERE id = 1",
                [],
                |row| {
                    let img: i32 = row.get(4)?;
                    let file: i32 = row.get(5)?;
                    let voice: i32 = row.get(7)?;
                    let video: i32 = row.get(8)?;
                    let req_pq: i32 = row.get(22)?;
                    let p2p: i32 = row.get(23)?;
                    let local_ch: i32 = row.get(26)?;
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
                        server_description: row.get(24)?,
                        server_name: row.get(25)?,
                        local_channel_enabled: local_ch != 0,
                        dm_mailbox_ttl_days: row.get::<_, i64>(27)?.max(1),
                        message_retention_days: row.get::<_, i64>(28)?.max(0),
                        erased_accounts_ttl_days: row.get::<_, i64>(29)?.max(1),
                        erased_accounts_cap: row.get::<_, i64>(30)?.max(1),
                        world_time_scale: clamp_world_time_scale(row.get::<_, f64>(31)?)
                            .unwrap_or_else(default_world_time_scale),
                        fleet_supply_mode: fleet_supply_mode_of(&row.get::<_, String>(32)?)
                            .unwrap_or(DEFAULT_FLEET_SUPPLY_MODE)
                            .to_string(),
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
                    server_description              = ?25,
                    server_name                     = ?26,
                    local_channel_enabled           = ?27,
                    dm_mailbox_ttl_days             = ?28,
                    message_retention_days          = ?29,
                    erased_accounts_ttl_days        = ?30,
                    erased_accounts_cap             = ?31,
                    world_time_scale                = ?32,
                    fleet_supply_mode               = ?33,
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
                    s.server_description,
                    s.server_name,
                    s.local_channel_enabled as i32,
                    s.dm_mailbox_ttl_days.max(1),
                    s.message_retention_days.max(0),
                    s.erased_accounts_ttl_days.max(1),
                    s.erased_accounts_cap.max(1),
                    clamp_world_time_scale(s.world_time_scale).unwrap_or_else(default_world_time_scale),
                    fleet_supply_mode_of(&s.fleet_supply_mode).unwrap_or(DEFAULT_FLEET_SUPPLY_MODE),
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

    /// v0.478: server_description must default to "" and round-trip through
    /// the positional set/get SQL (a shifted ?N index would silently corrupt an
    /// adjacent column, so this also re-checks two neighbors).
    #[test]
    fn server_description_roundtrips_and_defaults_empty() {
        let db = fresh_db();
        let s = db.get_server_settings().expect("get");
        assert_eq!(s.server_description, "", "MUST default empty");

        assert_eq!(s.server_name, "", "server_name MUST default empty too");

        let mut updated = s.clone();
        updated.server_description = "A cooperative homestead world. Be kind.".to_string();
        updated.server_name = "United Humanity".to_string();
        updated.p2p_distribution_enabled = true; // adjacent bool, catch index bleed
        updated.max_total_upload_mb = 777;       // an int, same row
        assert!(db.set_server_settings(&updated, "admin_key").expect("set"));

        let got = db.get_server_settings().expect("get2");
        assert_eq!(got.server_description, "A cooperative homestead world. Be kind.");
        assert_eq!(got.server_name, "United Humanity", "server_name must round-trip");
        assert!(got.p2p_distribution_enabled, "no positional-index bleed");
        assert_eq!(got.max_total_upload_mb, 777, "no positional-index bleed");
        assert_eq!(got.updated_by, "admin_key");
    }

    /// THE SHARED WORLD'S CLOCK SPEED (operator, 2026-10-04: 72x, adjustable
    /// in the app). A new server runs the shared world at the Simplified 72x
    /// (the schema's DEFAULT and the code's default are one number), an
    /// admin's value round-trips through the positional set/get SQL without
    /// bleeding into its neighbour, and a value outside the player setting's
    /// 1..=1000 is held to it; a value that is not a number is refused.
    ///
    /// Seen red 2026-10-04 with the get reading column 28 (the neighbour)
    /// for the speed: "assertion `left == right` failed: a new server runs
    /// the shared world at 72x, left: 1.0, right: 72.0" (retention's 0, held
    /// to the floor of 1).
    #[test]
    fn world_time_scale_defaults_to_simplified_and_round_trips() {
        let db = fresh_db();
        let s = db.get_server_settings().expect("get");
        assert_eq!(s.world_time_scale, 72.0, "a new server runs the shared world at 72x");
        assert_eq!(s.world_time_scale, default_world_time_scale(), "the schema DEFAULT is the Simplified speed");
        assert_eq!(ServerSettings::default().world_time_scale, 72.0);

        let mut updated = s.clone();
        updated.world_time_scale = 24.0;
        updated.message_retention_days = 90; // a neighbouring column
        updated.erased_accounts_cap = 4_321; // the column just before it (merged 2026-10-04)
        assert!(db.set_server_settings(&updated, "admin_key").expect("set"));
        let got = db.get_server_settings().expect("get2");
        assert_eq!(got.world_time_scale, 24.0, "the admin's speed persists");
        assert_eq!(got.message_retention_days, 90, "no positional-index bleed");
        assert_eq!(got.erased_accounts_cap, 4_321, "no positional-index bleed");

        updated.world_time_scale = 5000.0;
        assert!(db.set_server_settings(&updated, "admin_key").expect("set3"));
        assert_eq!(db.get_server_settings().expect("get3").world_time_scale, 1000.0, "held to the range");
        assert_eq!(clamp_world_time_scale(0.0), Some(1.0), "never slower than real time");
        assert_eq!(clamp_world_time_scale(f64::NAN), None, "not a number: refused");
    }

    /// THE FLEET'S SUPPLY SETTING (operator, 2026-10-04: "the fleet has unlimited of
    /// everything" during early development): a new server is "unlimited"; an admin's
    /// "stocked" persists through the positional SQL without bleeding into the clock's column
    /// beside it; a value that is no mode is refused by `fleet_supply_mode_of` and a stored one
    /// reads back as the default.
    ///
    /// Seen red 2026-10-04 with the UPDATE's ?33 bound to the default instead of the admin's
    /// mode: "the admin's mode persists / left: \"unlimited\" / right: \"stocked\"". (Binding ?32,
    /// the clock's, into the column instead is caught before any test can read it: rusqlite
    /// refuses the statement, "InvalidParameterCount(33, 32)".)
    #[test]
    fn fleet_supply_mode_defaults_to_unlimited_and_round_trips() {
        let db = fresh_db();
        let s = db.get_server_settings().expect("get");
        assert_eq!(s.fleet_supply_mode, "unlimited", "a new server's fleet is unlimited");
        assert_eq!(ServerSettings::default().fleet_supply_mode, DEFAULT_FLEET_SUPPLY_MODE);
        let mut updated = s.clone();
        updated.fleet_supply_mode = "stocked".into();
        updated.world_time_scale = 24.0; // the column just before it
        assert!(db.set_server_settings(&updated, "admin_key").expect("set"));
        let got = db.get_server_settings().expect("get2");
        assert_eq!(got.fleet_supply_mode, "stocked", "the admin's mode persists");
        assert_eq!(got.world_time_scale, 24.0, "no positional-index bleed");
        assert_eq!(fleet_supply_mode_of(" Stocked "), Some("stocked"));
        assert_eq!(fleet_supply_mode_of("plenty"), None, "not a mode: refused");
        db.with_conn(|c| c.execute("UPDATE server_settings SET fleet_supply_mode = 'plenty'", [])).unwrap();
        assert_eq!(db.get_server_settings().unwrap().fleet_supply_mode, "unlimited", "a stored value that is no mode reads as the default");
    }

    /// A server whose database predates the clock setting upgrades to 72x and
    /// keeps everything its owner had set (the BUG-046 shape: the live table
    /// already exists without the column).
    ///
    /// Seen red 2026-10-04 with the guarded ALTER switched off: "get_server_settings
    /// after the upgrade: SqlInputError { error: Error { code: Unknown,
    /// extended_code: 1 }, msg: \"no such column: world_time_scale\", ...".
    #[test]
    fn a_server_from_before_the_world_clock_upgrades_to_72x() {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_srvset_clock_{pid}_{nanos}.db"));
        {
            let db = Storage::open(&path).expect("open");
            let mut s = db.get_server_settings().expect("get");
            s.message_retention_days = 7;
            s.dm_mailbox_ttl_days = 10;
            assert!(db.set_server_settings(&s, "op_key").expect("set"));
        }
        {
            let conn = rusqlite::Connection::open(&path).expect("raw open");
            conn.execute_batch("ALTER TABLE server_settings DROP COLUMN world_time_scale;")
                .expect("drop column (SQLite >= 3.35)");
        }
        let db = Storage::open(&path).expect("reopen must not fail");
        let got = db.get_server_settings().expect("get_server_settings after the upgrade");
        assert_eq!(got.world_time_scale, 72.0, "the upgrade starts the clock at 72x");
        assert_eq!(got.message_retention_days, 7, "the owner's settings are kept");
        assert_eq!(got.dm_mailbox_ttl_days, 10);
        drop(db);
        let _ = std::fs::remove_file(&path);
    }

    /// The two erased-accounts settings (BUG-135, 2026-10-04) default to 30 days and 100,000
    /// entries and round-trip through the positional set/get SQL without bleeding into their
    /// neighbours (a shifted ?N would silently swap them, or the retention day count).
    ///
    /// Seen red 2026-10-04 with ?30 and ?31 swapped in the UPDATE: "assertion `left == right`
    /// failed: the window round-trips / left: 12345 / right: 12".
    #[test]
    fn erased_accounts_settings_default_and_roundtrip() {
        let db = fresh_db();
        let s = db.get_server_settings().expect("get");
        assert_eq!(s.erased_accounts_ttl_days, 30, "the window defaults to 30 days");
        assert_eq!(s.erased_accounts_cap, 100_000, "the cap defaults to 100,000");
        assert_eq!(ServerSettings::default().erased_accounts_ttl_days, 30);
        assert_eq!(ServerSettings::default().erased_accounts_cap, 100_000);
        let mut updated = s.clone();
        updated.erased_accounts_ttl_days = 12;
        updated.erased_accounts_cap = 12_345;
        updated.message_retention_days = 90; // the neighbour, to catch index bleed
        assert!(db.set_server_settings(&updated, "admin_key").expect("set"));
        let got = db.get_server_settings().expect("get2");
        assert_eq!(got.erased_accounts_ttl_days, 12, "the window round-trips");
        assert_eq!(got.erased_accounts_cap, 12_345, "the cap round-trips");
        assert_eq!(got.message_retention_days, 90, "no positional-index bleed");
        // An older client's settings JSON without the two fields still reads, with defaults.
        let mut v = serde_json::to_value(&got).unwrap();
        v.as_object_mut().unwrap().remove("erased_accounts_ttl_days");
        v.as_object_mut().unwrap().remove("erased_accounts_cap");
        let back: ServerSettings = serde_json::from_value(v).expect("reads without the two fields");
        assert_eq!((back.erased_accounts_ttl_days, back.erased_accounts_cap), (30, 100_000));
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
