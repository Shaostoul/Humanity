//! SQLite persistence for relay message history and peers.
//!
//! Security note: The security of the Humanity protocol lives in the
//! cryptographic object layer (Ed25519 signatures, XChaCha20-Poly1305
//! encryption), not in the storage layer. SQLite is just the container
//! for signed, tamper-evident objects.

use rand::Rng;
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use tracing::info;

use crate::relay::relay::RelayMessage;

/// A persisted DM record.
#[derive(Debug, Clone)]
pub struct DmRecord {
    pub from_key: String,
    pub from_name: String,
    pub to_key: String,
    pub content: String,
    pub timestamp: u64,
    /// Whether this DM is end-to-end encrypted.
    pub encrypted: bool,
    /// Base64-encoded nonce/IV for encrypted DMs.
    pub nonce: Option<String>,
}

/// A DM conversation summary.
#[derive(Debug, Clone)]
pub struct DmConversation {
    pub partner_key: String,
    pub partner_name: String,
    pub last_message: String,
    pub last_timestamp: u64,
    pub unread_count: i64,
}

/// A persisted pinned message record.
#[derive(Debug, Clone)]
pub struct PinnedMessageRecord {
    pub channel: String,
    pub from_key: String,
    pub from_name: String,
    pub content: String,
    pub original_timestamp: u64,
    pub pinned_by: String,
    pub pinned_at: u64,
}

/// A persisted emoji reaction record.
#[derive(Debug, Clone)]
pub struct ReactionRecord {
    pub target_from: String,
    pub target_timestamp: u64,
    pub emoji: String,
    pub reactor_key: String,
    pub reactor_name: String,
}

/// A project task record from the database.
#[derive(Debug, Clone)]
pub struct TaskRecord {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub assignee: Option<String>,
    pub created_by: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub position: i64,
    pub labels: String,
    pub project: String,
}

/// A task comment record from the database.
#[derive(Debug, Clone)]
pub struct TaskCommentRecord {
    pub id: i64,
    pub task_id: i64,
    pub author_key: String,
    pub author_name: String,
    pub content: String,
    pub created_at: i64,
}

/// A cached link preview record.
#[derive(Debug, Clone)]
pub struct LinkPreviewRecord {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    pub site_name: Option<String>,
    pub fetched_at: i64,
}

/// A voice channel record from the database.
#[derive(Debug, Clone)]
pub struct VoiceChannelRecord {
    pub id: i64,
    pub name: String,
    pub position: i64,
    pub created_by: Option<String>,
    pub created_at: i64,
}

/// A project record from the database (re-exported from projects.rs).
pub use projects::ProjectRecord;

/// A listing review record (re-exported from reviews.rs).
pub use reviews::ReviewRecord;

/// A server member record (re-exported from members.rs).
pub use members::MemberRecord;

/// A signed profile record (re-exported from signed_profiles.rs).
pub use signed_profiles::SignedProfileRecord;

/// A generic signed object record from the Phase 0 PQ substrate.
pub use signed_objects::{
    MAX_SIGNED_OBJECT_PAYLOAD, SignedObjectRecord, author_fingerprint, compute_object_id,
};

/// DID resolution: DID → current Dilithium3 pubkey + first/last-seen metadata.
pub use dids::DidResolution;

/// Verifiable Credential index row (Phase 1 PR 2).
pub use credentials::{CredentialIndex, extract_subject_did};

/// Multi-layer trust score (Phase 2 PR 1).
pub use trust_score::{SubScores, TrustInputs, TrustScore};

/// Governance: proposals + votes + tally (Phase 5 PR 1).
pub use governance::{ProposalIndex, ProposalTally, MAX_VOTE_WEIGHT};

/// AI-as-citizen status (Phase 8 PR 1).
pub use ai_status::{AiStatus, SubjectClass};

/// Social key recovery — Shamir share index + request + approval flow (Phase 4 PR 1+2).
pub use recovery::{RecoveryShareIndex, RecoverySetup, RecoveryRequestRecord, RecoveryApprovalRecord};

/// Per-observer per-issuer continuous trust (Phase 3 PR 2).
pub use issuer_trust::{IssuerTrustRow, NEUTRAL_TRUST, MAX_DELTA};

/// Multi-AI agent coordination tracking (v0.116.0).
pub use agent_sessions::{AgentSessionRow, CLAIM_TIMEOUT_SECS};

/// A marketplace listing record from the database.
#[derive(Debug, Clone)]
pub struct MarketplaceListing {
    pub id: String,
    pub seller_key: String,
    pub seller_name: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub category: String,
    pub condition: Option<String>,
    pub price: Option<String>,
    pub payment_methods: Option<String>,
    pub location: Option<String>,
    pub images: Option<String>,
    pub status: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// A listing image record from the database.
#[derive(Debug, Clone)]
pub struct ListingImage {
    pub id: i64,
    pub listing_id: String,
    pub url: String,
    pub position: i32,
    pub created_at: String,
}

/// A federated server record from the database.
#[derive(Debug, Clone)]
pub struct FederatedServer {
    pub server_id: String,
    pub name: String,
    pub url: String,
    pub public_key: Option<String>,
    pub trust_tier: i32,
    pub accord_compliant: bool,
    pub status: String,
    pub last_seen: Option<i64>,
    pub added_at: i64,
}

/// A push notification subscription record.
#[derive(Debug, Clone)]
pub struct PushSubscriptionRecord {
    pub public_key: String,
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
}

/// Persistent storage backed by SQLite.
///
/// ## Connection model (R3 concurrency work)
///
/// SQLite in WAL mode allows **one writer + many concurrent readers**. To get
/// that benefit we split the connections by role:
///
/// * `conn` — the SINGLE dedicated **writer**, behind a `Mutex`. Every write
///   (INSERT/UPDATE/DELETE/CREATE, transactions, `last_insert_rowid()`) goes
///   here, serialized, exactly as WAL requires. Reached via
///   [`Storage::with_conn`] and [`Storage::with_conn_mut`] — signatures
///   unchanged so the 30 storage modules' ~300 call sites compile untouched.
///
///   IMPORTANT: `with_conn` is used pervasively for WRITES today (e.g.
///   `dms::store_dm_e2ee` does `INSERT … ; last_insert_rowid()`), so it MUST
///   stay on the writer. Do not "optimize" it onto the read pool.
///
/// * `read_pool` — a small pool of **read-only** connections (see
///   [`pool`]). Reached via [`Storage::with_read_conn`]. A path may use this
///   ONLY if its closure is CERTAIN to be read-only; the pooled connections are
///   physically read-only, so a stray write fails loudly instead of corrupting.
///   This is the opt-in surface that lets independent reads run in parallel
///   instead of serializing on the writer mutex.
pub struct Storage {
    pub(crate) conn: Mutex<Connection>,
    /// Pool of read-only connections for concurrent reads. See [`pool`] and
    /// [`Storage::with_read_conn`].
    pub(crate) read_pool: pool::ReadPool,
}

/// Shared timestamp helper used by multiple submodules.
pub(crate) fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

impl Storage {
    /// Execute a closure with the **writer** connection (locked).
    ///
    /// This is the write path AND the default catch-all read path. It is used
    /// pervasively for writes today (INSERT/UPDATE/DELETE, and crucially
    /// `last_insert_rowid()` immediately after an INSERT — e.g.
    /// `dms::store_dm_e2ee`), so it MUST run on the single writer connection.
    /// Do NOT reroute this onto the read pool: a write through a read-only
    /// connection would fail, and `last_insert_rowid()` must be read on the
    /// same connection that did the INSERT.
    ///
    /// Reads that are provably read-only can opt into [`with_read_conn`] for
    /// concurrency; everything else stays here. Panics if the mutex is poisoned
    /// (same as the original unwrap behavior).
    pub(crate) fn with_conn<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&Connection) -> T,
    {
        let conn = self.conn.lock().unwrap();
        f(&conn)
    }

    /// Like `with_conn` but provides a mutable reference (needed for
    /// transactions). Also the writer connection.
    pub(crate) fn with_conn_mut<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&mut Connection) -> T,
    {
        let mut conn = self.conn.lock().unwrap();
        f(&mut conn)
    }

    /// Execute a **read-only** closure on a pooled connection, enabling
    /// concurrent reads (WAL allows many simultaneous readers).
    ///
    /// Use this ONLY when the closure is CERTAIN to be read-only (`SELECT` /
    /// `PRAGMA` reads / `query_row` / `query_map`). The pooled connections are
    /// opened `SQLITE_OPEN_READ_ONLY`, so any write attempted through here fails
    /// at the SQLite layer ("attempt to write a readonly database") rather than
    /// corrupting data — a mis-route is caught, not silent. When in doubt, use
    /// [`with_conn`] (the writer); erring toward the writer is always correct,
    /// just less parallel.
    ///
    /// On pool exhaustion (all connections checked out longer than the pool's
    /// checkout timeout) this returns a `SQLITE_BUSY` error rather than
    /// panicking, so a caller can degrade gracefully or fall back to
    /// [`with_conn`]. The closure therefore receives `Result<&Connection, _>`
    /// only indirectly: the pool-acquisition error is surfaced as the closure's
    /// `T` is wrapped — see the signature. To keep call sites simple, callers
    /// get the connection directly and any pool error is mapped into a
    /// `rusqlite::Error`; closures that already return `Result<_, rusqlite::Error>`
    /// compose cleanly via `?`.
    #[allow(dead_code)] // Opt-in accessor: read-heavy paths adopt it incrementally.
    pub(crate) fn with_read_conn<F, T>(&self, f: F) -> Result<T, rusqlite::Error>
    where
        F: FnOnce(&Connection) -> Result<T, rusqlite::Error>,
    {
        // Acquire a read connection from the pool. A checkout failure (pool
        // exhausted past the timeout, or a connection failed validation) is
        // folded into the rusqlite error channel as SQLITE_BUSY so callers can
        // treat it like any other transient DB error / fall back to the writer.
        let conn = self.read_pool.get().map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
                Some(format!("read pool checkout failed: {e}")),
            )
        })?;
        f(&conn)
    }

    /// Open the DB with corruption detection + backup-restore recovery.
    ///
    /// This is the boot-path entry point the relay should use (the plain
    /// `open()` below is unchanged and used everywhere else / by tests).
    ///
    /// Healthy path = `open()` + one fast `PRAGMA quick_check`. Identical
    /// behavior to `open()` for a healthy DB; the only addition is the
    /// integrity probe.
    ///
    /// On detected corruption (open error OR failed quick_check):
    ///   1. Walk `backups_dir`'s `relay-*.db` newest-first, verifying each
    ///      READ-ONLY (no mutation of the backup) until one passes
    ///      quick_check.
    ///   2. The first healthy backup: quarantine the corrupt live file(s)
    ///      to `<path>.corrupt-<ts>` (preserved for forensics, NOT
    ///      deleted), copy the backup into place, open + verify it.
    ///   3. If NO backup verifies clean: DO NOT touch the live file and
    ///      return Err. The relay then refuses to start (the boot site
    ///      `.expect()`s) — a loud, visible failure the watchdog flags —
    ///      rather than silently running on corrupt data OR silently
    ///      creating a fresh empty schema (which would masquerade as a
    ///      wipe). Refusing-to-start is the safe failure for an
    ///      unattended relay: no surprise data loss; operator decides.
    ///
    /// Added after the 2026-05-21 incident review (TIER 1 #3 SQLite WAL
    /// corruption recovery). See docs/INCIDENT-PLAYBOOK.md.
    pub fn open_resilient(path: &Path, backups_dir: &Path) -> Result<Self, rusqlite::Error> {
        // Healthy path (the overwhelming common case).
        match Self::open_and_verify(path) {
            Ok(s) => return Ok(s),
            Err(e) => {
                tracing::error!(
                    "DB at {} failed to open or passed integrity check ({}). Entering recovery.",
                    path.display(), e
                );
            }
        }

        // Recovery: newest-first, restore the first backup that verifies.
        let candidates = Self::list_backups(backups_dir);
        tracing::warn!("Recovery: {} backup candidate(s) in {}", candidates.len(), backups_dir.display());
        for backup in &candidates {
            if !Self::verify_readonly(backup) {
                tracing::warn!("Recovery: backup {} also fails integrity — trying older", backup.display());
                continue;
            }
            // Healthy backup found. Quarantine the corrupt live file(s),
            // copy the backup into place, open + verify.
            Self::quarantine_corrupt(path);
            match std::fs::copy(backup, path) {
                Ok(_) => match Self::open_and_verify(path) {
                    Ok(s) => {
                        tracing::warn!("DB RECOVERED from backup {}", backup.display());
                        return Ok(s);
                    }
                    Err(e) => {
                        tracing::error!("Restored backup {} failed post-copy verify: {}", backup.display(), e);
                        // Fall through to the loud-failure path below.
                        break;
                    }
                },
                Err(e) => {
                    tracing::error!("Failed to copy backup {} into place: {}", backup.display(), e);
                    break;
                }
            }
        }

        // No healthy backup (or post-copy verify failed). Fail loud —
        // refuse to start rather than silently wipe or run corrupt.
        tracing::error!(
            "DB recovery FAILED: no healthy backup in {}. Refusing to start on corrupt data — operator intervention required (restore a known-good backup, or run scripts/pq-wipe.sh for an intentional fresh slate).",
            backups_dir.display()
        );
        Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CORRUPT),
            Some(format!("DB at {} corrupt and no healthy backup in {}", path.display(), backups_dir.display())),
        ))
    }

    /// `open()` plus a fast `PRAGMA quick_check`. Returns Err if the open
    /// fails OR the integrity probe is not "ok". Used for the live path
    /// (migrations run, which is correct there).
    fn open_and_verify(path: &Path) -> Result<Self, rusqlite::Error> {
        let storage = Self::open(path)?;
        let ok: String = storage.with_conn(|c| {
            c.query_row("PRAGMA quick_check", [], |r| r.get::<_, String>(0))
        })?;
        if ok != "ok" {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CORRUPT),
                Some(format!("quick_check returned '{}'", ok)),
            ));
        }
        Ok(storage)
    }

    /// Verify a candidate backup WITHOUT mutating it: open read-only and
    /// run quick_check. Read-only matters — we must not run migrations or
    /// touch the WAL of a backup we might not even use.
    fn verify_readonly(path: &Path) -> bool {
        use rusqlite::OpenFlags;
        let conn = match Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            Ok(c) => c,
            Err(_) => return false,
        };
        matches!(
            conn.query_row("PRAGMA quick_check", [], |r| r.get::<_, String>(0)),
            Ok(s) if s == "ok"
        )
    }

    /// List `relay-*.db` backups in `backups_dir`, newest-first by mtime.
    fn list_backups(backups_dir: &Path) -> Vec<std::path::PathBuf> {
        let mut entries: Vec<(std::time::SystemTime, std::path::PathBuf)> = std::fs::read_dir(backups_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                let name = p.file_name()?.to_str()?;
                if name.starts_with("relay-") && name.ends_with(".db") {
                    let mtime = e.metadata().ok()?.modified().ok()?;
                    Some((mtime, p))
                } else {
                    None
                }
            })
            .collect();
        entries.sort_by(|a, b| b.0.cmp(&a.0)); // newest first
        entries.into_iter().map(|(_, p)| p).collect()
    }

    /// Move a corrupt DB file (plus its -wal/-shm sidecars) aside to
    /// `<path>.corrupt-<ts>` for forensics. Best-effort; never panics.
    fn quarantine_corrupt(path: &Path) {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        for suffix in ["", "-wal", "-shm"] {
            let src = std::path::PathBuf::from(format!("{}{}", path.display(), suffix));
            if src.exists() {
                let dst = std::path::PathBuf::from(format!("{}.corrupt-{}{}", path.display(), ts, suffix));
                if let Err(e) = std::fs::rename(&src, &dst) {
                    tracing::warn!("quarantine: could not move {} aside: {}", src.display(), e);
                } else {
                    tracing::warn!("quarantine: moved {} -> {}", src.display(), dst.display());
                }
            }
        }
    }

    /// Open or create the database at the given path.
    pub fn open(path: &Path) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;

        // Enable WAL mode for better concurrent read/write performance.
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS messages (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                msg_type  TEXT NOT NULL,
                from_key  TEXT,
                from_name TEXT,
                content   TEXT,
                timestamp INTEGER NOT NULL,
                signature TEXT,
                raw_json  TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_messages_timestamp
                ON messages(timestamp);

            -- Add channel_id column if it doesn't exist (migration).
            -- SQLite doesn't have IF NOT EXISTS for ALTER TABLE, so we handle it in code.

            CREATE TABLE IF NOT EXISTS peers (
                public_key  TEXT PRIMARY KEY,
                display_name TEXT,
                last_seen   INTEGER NOT NULL
            );

            -- Full-PQ cutover (v0.262.33): `public_key` is the
            -- Dilithium3 public-key hex (THE identity; DID derives from
            -- it). Ed25519 is Solana-wallet-only, never reaches the
            -- relay. `kyber_public` = recipient ML-KEM-768 encapsulation
            -- key (base64) for E2EE DMs. No ecdh/dilithium_public cols
            -- (dual-stack scaffolding trimmed).
            CREATE TABLE IF NOT EXISTS registered_names (
                name        TEXT NOT NULL COLLATE NOCASE,
                public_key  TEXT NOT NULL,
                kyber_public TEXT DEFAULT NULL,
                registered_at INTEGER NOT NULL,
                PRIMARY KEY (name, public_key)
            );

            CREATE INDEX IF NOT EXISTS idx_registered_names_name
                ON registered_names(name COLLATE NOCASE);

            CREATE TABLE IF NOT EXISTS link_codes (
                code        TEXT PRIMARY KEY,
                name        TEXT NOT NULL COLLATE NOCASE,
                created_by  TEXT NOT NULL,
                created_at  INTEGER NOT NULL,
                expires_at  INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS channels (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                description TEXT,
                created_by  TEXT,
                created_at  INTEGER NOT NULL,
                read_only   INTEGER DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS invite_codes (
                code        TEXT PRIMARY KEY,
                created_by  TEXT NOT NULL,
                created_at  INTEGER NOT NULL,
                expires_at  INTEGER NOT NULL,
                used_by     TEXT
            );

            CREATE TABLE IF NOT EXISTS user_roles (
                public_key  TEXT PRIMARY KEY,
                role        TEXT NOT NULL DEFAULT 'user'
            );

            CREATE TABLE IF NOT EXISTS banned_keys (
                public_key  TEXT PRIMARY KEY,
                banned_at   INTEGER NOT NULL,
                name        TEXT NOT NULL DEFAULT ''
            );

            -- v0.246: mute is orthogonal to roles. The OLD /mute set
            -- user_roles.role=muted which clobbered the user real role
            -- (donor/verified/mod) and /unmute reset it to a bogus user
            -- role (data loss). This table records a mute WITHOUT
            -- touching the role, so unmute leaves the original role
            -- intact. The name column is captured so the mod Muted-users
            -- panel can show who it is. Created fresh on every startup
            -- so existing DBs pick it up with no ALTER needed.
            CREATE TABLE IF NOT EXISTS muted_members (
                public_key  TEXT PRIMARY KEY,
                muted_at    INTEGER NOT NULL,
                name        TEXT NOT NULL DEFAULT ''
            );

            CREATE TABLE IF NOT EXISTS user_uploads (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                public_key  TEXT NOT NULL,
                filename    TEXT NOT NULL,
                uploaded_at INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_user_uploads_key
                ON user_uploads(public_key, id);

            CREATE TABLE IF NOT EXISTS reports (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                reporter_key TEXT NOT NULL,
                reported_name TEXT NOT NULL,
                reason      TEXT NOT NULL DEFAULT '',
                created_at  INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS reactions (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                target_from     TEXT NOT NULL,
                target_timestamp INTEGER NOT NULL,
                emoji           TEXT NOT NULL,
                reactor_key     TEXT NOT NULL,
                reactor_name    TEXT NOT NULL DEFAULT '',
                channel         TEXT NOT NULL DEFAULT 'general',
                created_at      INTEGER NOT NULL,
                UNIQUE(target_from, target_timestamp, emoji, reactor_key)
            );

            CREATE INDEX IF NOT EXISTS idx_reactions_target
                ON reactions(target_from, target_timestamp);
            CREATE INDEX IF NOT EXISTS idx_reactions_channel
                ON reactions(channel);

            CREATE TABLE IF NOT EXISTS pinned_messages (
                id                INTEGER PRIMARY KEY AUTOINCREMENT,
                channel           TEXT NOT NULL,
                from_key          TEXT NOT NULL,
                from_name         TEXT NOT NULL,
                content           TEXT NOT NULL,
                original_timestamp INTEGER NOT NULL,
                pinned_by         TEXT NOT NULL,
                pinned_at         INTEGER NOT NULL,
                UNIQUE(channel, from_key, original_timestamp)
            );

            CREATE INDEX IF NOT EXISTS idx_pinned_channel
                ON pinned_messages(channel);"
        )?;

        // Server state key-value store (for persisting lockdown, etc.).
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS server_state (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );"
        )?;

        // ── Game-state persistence (relay game-world durability) ──
        // The server-authoritative GameWorld (entities/positions/game_time)
        // and per-player quest/XP/reputation used to live ONLY in memory and
        // reset on every relay restart. These two tables make them durable.
        // See storage/game_persistence.rs for the full rationale.
        //
        //   game_world_snapshots — one row per logical world. snapshot_json is
        //   the opaque serialized world blob (entities + the two scalars);
        //   game_time + next_entity_id are also broken out as columns for cheap
        //   inspection without parsing the JSON.
        //
        //   player_progress — one row per player (their Dilithium pubkey hex).
        //   completed_quests is a JSON array string (SQLite has no array type,
        //   matching how project_tasks.labels is stored).
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS game_world_snapshots (
                world_id        TEXT PRIMARY KEY,
                snapshot_json   TEXT NOT NULL,
                game_time       REAL NOT NULL DEFAULT 0,
                next_entity_id  INTEGER NOT NULL DEFAULT 1,
                updated_at      INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS player_progress (
                public_key       TEXT PRIMARY KEY,
                current_quest    TEXT,
                completed_quests TEXT NOT NULL DEFAULT '[]',
                xp               INTEGER NOT NULL DEFAULT 0,
                reputation       INTEGER NOT NULL DEFAULT 0,
                updated_at       INTEGER NOT NULL DEFAULT 0
            );"
        )?;

        // Federation: federated server registry.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS federated_servers (
                server_id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                url TEXT NOT NULL,
                public_key TEXT,
                trust_tier INTEGER NOT NULL DEFAULT 0,
                accord_compliant INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'unknown',
                last_seen INTEGER,
                added_at INTEGER NOT NULL
            );"
        )?;

        // DM table for direct messages.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS direct_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                from_key TEXT NOT NULL,
                from_name TEXT NOT NULL,
                to_key TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                read INTEGER NOT NULL DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_dm_conversation
                ON direct_messages(from_key, to_key);
            CREATE INDEX IF NOT EXISTS idx_dm_to
                ON direct_messages(to_key);"
        )?;

        // Migration: add channel_id column to messages if missing.
        let has_channel_id: bool = conn
            .prepare("SELECT channel_id FROM messages LIMIT 0")
            .is_ok();
        if !has_channel_id {
            conn.execute_batch(
                "ALTER TABLE messages ADD COLUMN channel_id TEXT DEFAULT 'general';"
            )?;
            info!("Migration: added channel_id column to messages");
        }

        // Create index on channel_id for efficient per-channel queries.
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_messages_channel ON messages(channel_id, id);"
        )?;

        // User data sync table (settings, notes, todos, etc.).
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS user_data (
                public_key TEXT PRIMARY KEY,
                data BLOB NOT NULL,
                updated_at INTEGER NOT NULL
            );"
        )?;

        // Migration: user_status table.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS user_status (
                name TEXT PRIMARY KEY COLLATE NOCASE,
                status TEXT NOT NULL DEFAULT 'online',
                status_text TEXT NOT NULL DEFAULT ''
            );"
        )?;

        // Migration: profiles table for user bios and social links.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS profiles (
                name    TEXT PRIMARY KEY COLLATE NOCASE,
                bio     TEXT NOT NULL DEFAULT '',
                socials TEXT NOT NULL DEFAULT '{}'
            );"
        )?;

        // Migration: add extended profile columns if the table was created without them.
        // SQLite ALTER TABLE ADD COLUMN is safe to run if the column already exists — it will
        // return an error that we intentionally ignore by using execute_batch's best-effort mode.
        for alter in &[
            "ALTER TABLE profiles ADD COLUMN avatar_url TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE profiles ADD COLUMN banner_url  TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE profiles ADD COLUMN pronouns    TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE profiles ADD COLUMN location    TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE profiles ADD COLUMN website     TEXT NOT NULL DEFAULT ''",
            // privacy is a JSON map: {"location":"private", ...}  — default = all public.
            "ALTER TABLE profiles ADD COLUMN privacy     TEXT NOT NULL DEFAULT '{}'",
        ] {
            let _ = conn.execute(alter, []);
        }

        // Channel categories for Discord-like grouping.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS channel_categories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                position INTEGER NOT NULL DEFAULT 0,
                collapsed INTEGER NOT NULL DEFAULT 0
            );"
        )?;

        // Persistent voice channels.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS voice_channels (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                name        TEXT NOT NULL,
                position    INTEGER DEFAULT 0,
                created_by  TEXT,
                created_at  INTEGER NOT NULL
            );"
        )?;

        // Link preview cache for URL embeds.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS link_previews (
                url TEXT PRIMARY KEY,
                title TEXT,
                description TEXT,
                image TEXT,
                site_name TEXT,
                fetched_at INTEGER NOT NULL
            );"
        )?;

        // Project board: tasks (kanban).
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS project_tasks (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                title       TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                status      TEXT NOT NULL DEFAULT 'backlog',
                priority    TEXT NOT NULL DEFAULT 'medium',
                assignee    TEXT,
                created_by  TEXT NOT NULL,
                created_at  INTEGER NOT NULL,
                updated_at  INTEGER NOT NULL,
                position    INTEGER NOT NULL DEFAULT 0,
                labels      TEXT NOT NULL DEFAULT '[]'
            );

            CREATE INDEX IF NOT EXISTS idx_project_tasks_status
                ON project_tasks(status);

            CREATE TABLE IF NOT EXISTS task_comments (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id     INTEGER NOT NULL,
                author_key  TEXT NOT NULL,
                author_name TEXT NOT NULL,
                content     TEXT NOT NULL,
                created_at  INTEGER NOT NULL,
                FOREIGN KEY (task_id) REFERENCES project_tasks(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_task_comments_task
                ON task_comments(task_id);"
        )?;

        // Migration: add read_only column to channels if missing.
        let has_read_only: bool = conn
            .prepare("SELECT read_only FROM channels LIMIT 0")
            .is_ok();
        if !has_read_only {
            conn.execute_batch(
                "ALTER TABLE channels ADD COLUMN read_only INTEGER DEFAULT 0;"
            )?;
            info!("Migration: added read_only column to channels");
        }

        // Migration: add position column to channels if missing.
        let has_position: bool = conn
            .prepare("SELECT position FROM channels LIMIT 0")
            .is_ok();
        if !has_position {
            conn.execute_batch(
                "ALTER TABLE channels ADD COLUMN position INTEGER DEFAULT 100;"
            )?;
            info!("Migration: added position column to channels");
        }

        // Migration: add category_id column to channels if missing.
        let has_category_id: bool = conn
            .prepare("SELECT category_id FROM channels LIMIT 0")
            .is_ok();
        if !has_category_id {
            conn.execute_batch(
                "ALTER TABLE channels ADD COLUMN category_id INTEGER DEFAULT NULL;"
            )?;
            info!("Migration: added category_id column to channels");
        }

        // Full-PQ cutover (v0.262.33): the ecdh_public + dilithium_public
        // dual-stack ALTERs are GONE. `public_key` is the Dilithium
        // identity; `kyber_public` is in the CREATE TABLE above. This
        // idempotent ALTER only covers a pre-cutover DB; `just pq-wipe
        // yes` makes it irrelevant (fresh schema).
        let has_kyber: bool = conn
            .prepare("SELECT kyber_public FROM registered_names LIMIT 0")
            .is_ok();
        if !has_kyber {
            conn.execute_batch(
                "ALTER TABLE registered_names ADD COLUMN kyber_public TEXT DEFAULT NULL;"
            )?;
            info!("Migration: added kyber_public column to registered_names");
        }

        // Migration: add encrypted and nonce columns to direct_messages for E2EE.
        let has_dm_encrypted: bool = conn
            .prepare("SELECT encrypted FROM direct_messages LIMIT 0")
            .is_ok();
        if !has_dm_encrypted {
            conn.execute_batch(
                "ALTER TABLE direct_messages ADD COLUMN encrypted INTEGER DEFAULT 0;
                 ALTER TABLE direct_messages ADD COLUMN nonce TEXT DEFAULT NULL;"
            )?;
            info!("Migration: added encrypted/nonce columns to direct_messages");
        }

        // Migration: add reply_to columns to messages for threaded replies.
        let has_reply_to: bool = conn
            .prepare("SELECT reply_to_from FROM messages LIMIT 0")
            .is_ok();
        if !has_reply_to {
            conn.execute_batch(
                "ALTER TABLE messages ADD COLUMN reply_to_from TEXT DEFAULT NULL;
                 ALTER TABLE messages ADD COLUMN reply_to_timestamp INTEGER DEFAULT NULL;"
            )?;
            info!("Migration: added reply_to_from/reply_to_timestamp columns to messages");
        }

        // Index for efficient thread queries (find all replies to a message).
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_messages_reply_to
                ON messages(reply_to_from, reply_to_timestamp);"
        )?;

        // Migration: add label column to registered_names for device labeling.
        if conn.prepare("SELECT label FROM registered_names LIMIT 0").is_err() {
            conn.execute_batch(
                "ALTER TABLE registered_names ADD COLUMN label TEXT DEFAULT NULL;"
            )?;
            info!("Migration: added label column to registered_names");
        }

        // Migration: add federated column to channels for federation phase 2.
        if conn.prepare("SELECT federated FROM channels LIMIT 0").is_err() {
            conn.execute_batch(
                "ALTER TABLE channels ADD COLUMN federated INTEGER DEFAULT 0;"
            )?;
            info!("Migration: added federated column to channels");
        }

        // Migration: add voice_enabled column to channels (v0.192.0).
        // Default 1 = voice ON for all existing channels (matches the
        // pre-migration behavior where voice was implicitly always
        // available). Server Settings → Channels admin can toggle it
        // per-channel via channel_update.
        if conn.prepare("SELECT voice_enabled FROM channels LIMIT 0").is_err() {
            conn.execute_batch(
                "ALTER TABLE channels ADD COLUMN voice_enabled INTEGER DEFAULT 1;"
            )?;
            info!("Migration: added voice_enabled column to channels");
        }

        // ── Server settings singleton table (v0.200.0) ──
        // One row per server, identified by id=1. Holds operator-tunable
        // policies that previously lived as compile-time constants or
        // were entirely missing. UI exposes these in Server Settings →
        // Admin section. WS protocol: server_settings_request /
        // server_settings_update broadcast a fresh server_settings_state
        // to all clients on change.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS server_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                max_chars_unverified      INTEGER NOT NULL DEFAULT 280,
                max_chars_verified        INTEGER NOT NULL DEFAULT 1000,
                max_chars_mod             INTEGER NOT NULL DEFAULT 4000,
                max_chars_admin           INTEGER NOT NULL DEFAULT 10000,
                image_sharing_enabled     INTEGER NOT NULL DEFAULT 1,
                file_sharing_enabled      INTEGER NOT NULL DEFAULT 1,
                max_upload_mb             INTEGER NOT NULL DEFAULT 25,
                voice_channels_enabled    INTEGER NOT NULL DEFAULT 1,
                video_streaming_enabled   INTEGER NOT NULL DEFAULT 0,
                allowed_file_extensions   TEXT    NOT NULL DEFAULT 'png,jpg,jpeg,gif,webp,pdf,txt,md',
                max_uploads_per_user      INTEGER NOT NULL DEFAULT 4,
                max_total_upload_mb       INTEGER NOT NULL DEFAULT 500,
                max_uploads_per_user_unverified INTEGER NOT NULL DEFAULT 4,
                max_uploads_per_user_verified   INTEGER NOT NULL DEFAULT 20,
                max_uploads_per_user_mod        INTEGER NOT NULL DEFAULT 100,
                max_uploads_per_user_admin      INTEGER NOT NULL DEFAULT 500,
                require_pq_signatures           INTEGER NOT NULL DEFAULT 0,
                p2p_distribution_enabled        INTEGER NOT NULL DEFAULT 0,
                updated_at                INTEGER NOT NULL DEFAULT 0,
                updated_by                TEXT
            );
            INSERT OR IGNORE INTO server_settings (id) VALUES (1);"
        )?;

        // ── v0.237.0 — upload-storage limits became tunable ──
        // Operator: "we seem to have a limit to how many images the
        // server stores ... we should have whatever the variable is in
        // the server settings page." The per-user FIFO retention (was a
        // hardcoded 4 in storage/uploads.rs) and the server-wide disk
        // cap (was a hardcoded 500 MB in relay/api.rs) are now columns.
        // Idempotent ALTER for relays created before this migration.
        if conn.prepare("SELECT max_uploads_per_user FROM server_settings LIMIT 0").is_err() {
            conn.execute_batch(
                "ALTER TABLE server_settings ADD COLUMN max_uploads_per_user INTEGER NOT NULL DEFAULT 4;
                 ALTER TABLE server_settings ADD COLUMN max_total_upload_mb  INTEGER NOT NULL DEFAULT 500;"
            )?;
            info!("Migration: added max_uploads_per_user + max_total_upload_mb (server_settings)");
        }

        // ── v0.253.0 — PQ Increment 3: gated PQ-signature enforcement ──
        // require_pq_signatures defaults 0 (OFF) so existing relays keep
        // accepting Ed25519-only exactly as before until the operator
        // explicitly opts in. Idempotent ALTER for pre-v0.253 relays.
        if conn.prepare("SELECT require_pq_signatures FROM server_settings LIMIT 0").is_err() {
            conn.execute_batch(
                "ALTER TABLE server_settings ADD COLUMN require_pq_signatures INTEGER NOT NULL DEFAULT 0;"
            )?;
            info!("Migration: added require_pq_signatures (server_settings)");
        }

        // ── v0.262.16 — Server→Services: P2P-distribution soft gate ──
        // Soft toggle for the future BitTorrent model-distribution
        // feature. DEFAULT 0 (OFF) so existing relays are unaffected
        // (the feature isn't built; this is plumbing + a no-op gate).
        // Idempotent guarded ALTER, run BEFORE any SELECT of the column
        // (get_server_settings now reads it) — the 2026-05-17
        // migration-ordering lesson. The server_settings seed is
        // `(id) VALUES (1)` (never names columns) so the seed-names-new-
        // column failure class cannot occur here; the guarded ALTER is
        // purely for pre-v0.262.16 databases.
        if conn.prepare("SELECT p2p_distribution_enabled FROM server_settings LIMIT 0").is_err() {
            conn.execute_batch(
                "ALTER TABLE server_settings ADD COLUMN p2p_distribution_enabled INTEGER NOT NULL DEFAULT 0;"
            )?;
            info!("Migration: added p2p_distribution_enabled (server_settings)");
        }

        // ── v0.238.0 — per-role FIFO retention ──
        // Operator: "expand the sensible settings to include per ranking
        // such as unverified, verified, mod, admin." Same idempotent
        // ALTER pattern. On a relay that already had the v0.237 single
        // max_uploads_per_user column, forward a customized value (non-
        // default, != 4) into all four per-role columns so the operator's
        // tuning isn't lost.
        if conn.prepare("SELECT max_uploads_per_user_unverified FROM server_settings LIMIT 0").is_err() {
            conn.execute_batch(
                "ALTER TABLE server_settings ADD COLUMN max_uploads_per_user_unverified INTEGER NOT NULL DEFAULT 4;
                 ALTER TABLE server_settings ADD COLUMN max_uploads_per_user_verified   INTEGER NOT NULL DEFAULT 20;
                 ALTER TABLE server_settings ADD COLUMN max_uploads_per_user_mod        INTEGER NOT NULL DEFAULT 100;
                 ALTER TABLE server_settings ADD COLUMN max_uploads_per_user_admin      INTEGER NOT NULL DEFAULT 500;"
            )?;
            let prev: i64 = conn.query_row(
                "SELECT max_uploads_per_user FROM server_settings WHERE id = 1",
                [],
                |row| row.get(0),
            ).unwrap_or(4);
            if prev != 4 {
                conn.execute(
                    "UPDATE server_settings SET
                        max_uploads_per_user_unverified = ?1,
                        max_uploads_per_user_verified   = ?1,
                        max_uploads_per_user_mod        = ?1,
                        max_uploads_per_user_admin      = ?1
                     WHERE id = 1",
                    rusqlite::params![prev],
                )?;
                info!("Migration: forwarded v0.237 max_uploads_per_user={} into all 4 per-role columns", prev);
            }
            info!("Migration: split max_uploads_per_user into per-role columns (server_settings)");
        }

        // ── v0.239.0 — data-driven roles table ──
        // See docs/design/roles-system.md. Creates `roles` + seeds the
        // 5 built-ins (unverified/verified/donor/mod/admin). Idempotent
        // (INSERT OR IGNORE preserves operator-tuned seed capabilities).
        // Inlined here (not a Storage method) because we're already
        // inside the migration's `conn` closure — calling a method that
        // re-enters with_conn would re-borrow the connection.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS roles (
                id          TEXT PRIMARY KEY,
                label       TEXT NOT NULL,
                color       TEXT NOT NULL,
                trust_level INTEGER NOT NULL,
                built_in    INTEGER NOT NULL,
                can_stream  INTEGER NOT NULL,
                can_upload  INTEGER NOT NULL,
                can_voice   INTEGER NOT NULL,
                base_tier   TEXT NOT NULL,
                sort_order  INTEGER NOT NULL DEFAULT 0,
                can_image_share INTEGER NOT NULL DEFAULT 1,
                can_file_share  INTEGER NOT NULL DEFAULT 1,
                max_chars        INTEGER NOT NULL DEFAULT 280,
                max_upload_mb    INTEGER NOT NULL DEFAULT 5,
                max_uploads_kept INTEGER NOT NULL DEFAULT 4
             );",
        )?;
        // CRITICAL ORDERING (fixed v0.262.2 — production incident
        // 2026-05-17): the guarded ALTERs below MUST run BEFORE the seed
        // INSERT. On an existing relay.db the `CREATE TABLE IF NOT
        // EXISTS` above is a no-op, so the new columns
        // (can_image_share / max_chars / …) only exist after these
        // ALTERs. v0.261/v0.262 wrongly seeded first → on every live DB
        // the seed INSERT referenced a missing column → panic →
        // crash-loop → relay down. Fresh DBs were unaffected (CREATE
        // makes the columns) which is exactly why the bug shipped: the
        // upgrade path was never exercised. ALTER-then-seed is correct
        // for BOTH paths.

        // ── v0.261.0 — per-role image/file sharing capability ──
        // Operator: make image/file sharing per-role like streaming, so
        // effective = server master AND role capability. DEFAULT 1 so
        // every EXISTING role keeps sharing (sharing stays gated only by
        // the server-wide toggle exactly as before) — the upgrade is
        // non-breaking; the per-role denial is purely additive/opt-in.
        if conn.prepare("SELECT can_image_share FROM roles LIMIT 0").is_err() {
            conn.execute_batch(
                "ALTER TABLE roles ADD COLUMN can_image_share INTEGER NOT NULL DEFAULT 1;
                 ALTER TABLE roles ADD COLUMN can_file_share  INTEGER NOT NULL DEFAULT 1;"
            )?;
            info!("Migration: added can_image_share + can_file_share (roles)");
        }

        // ── v0.262.0 — R4: per-role numeric limits (operator-requested) ──
        // The Per-role-limits matrix + Roles grid merge into ONE table:
        // each role now OWNS max_chars / max_upload_mb / max_uploads_kept
        // instead of inheriting them from a server_settings tier via
        // base_tier. NON-BREAKING upgrade: after adding the columns,
        // backfill every existing role from the LIVE server_settings row
        // using its old base_tier, so each role keeps the EXACT effective
        // numbers it had pre-R4. base_tier is retained only as the
        // migration source + the add-form "prefill from preset"
        // convenience; it is no longer a runtime indirection.
        if conn.prepare("SELECT max_chars FROM roles LIMIT 0").is_err() {
            conn.execute_batch(
                "ALTER TABLE roles ADD COLUMN max_chars        INTEGER NOT NULL DEFAULT 280;
                 ALTER TABLE roles ADD COLUMN max_upload_mb    INTEGER NOT NULL DEFAULT 5;
                 ALTER TABLE roles ADD COLUMN max_uploads_kept INTEGER NOT NULL DEFAULT 4;"
            )?;
            // Backfill from the live server_settings tier the role used
            // to point at — behaviour is identical post-upgrade. (On a
            // fresh DB this whole block is skipped — CREATE already made
            // max_chars — so the seed below provides the numbers.)
            conn.execute_batch(
                "UPDATE roles SET
                   max_chars = (SELECT CASE roles.base_tier
                       WHEN 'verified' THEN s.max_chars_verified
                       WHEN 'mod'      THEN s.max_chars_mod
                       WHEN 'admin'    THEN s.max_chars_admin
                       ELSE s.max_chars_unverified END
                     FROM server_settings s WHERE s.id = 1),
                   max_upload_mb = (SELECT CASE roles.base_tier
                       WHEN 'verified' THEN s.max_upload_mb_verified
                       WHEN 'mod'      THEN s.max_upload_mb_mod
                       WHEN 'admin'    THEN s.max_upload_mb_admin
                       ELSE s.max_upload_mb_unverified END
                     FROM server_settings s WHERE s.id = 1),
                   max_uploads_kept = (SELECT CASE roles.base_tier
                       WHEN 'verified' THEN s.max_uploads_per_user_verified
                       WHEN 'mod'      THEN s.max_uploads_per_user_mod
                       WHEN 'admin'    THEN s.max_uploads_per_user_admin
                       ELSE s.max_uploads_per_user_unverified END
                     FROM server_settings s WHERE s.id = 1);"
            )?;
            info!("Migration: R4 — added per-role max_chars/max_upload_mb/max_uploads_kept + backfilled from base_tier");
        }

        // Seed the 5 built-ins. Runs AFTER the ALTERs above so every
        // column exists on both fresh and upgraded DBs. INSERT OR IGNORE
        // → existing built-ins (already backfilled above on an upgrade)
        // are preserved untouched; only genuinely-missing rows insert.
        {
            // Tuple tail (mc,mu,mk) = the canonical historical per-tier
            // numbers (chars / upload MB / uploads-kept) each built-in
            // used to inherit via base_tier — now owned per-role (R4,
            // v0.262). Fresh DBs seed these directly; upgrades keep the
            // backfilled values (rows already exist → IGNORE).
            let seeds: &[(&str, &str, &str, i64, i64, i64, i64, &str, i64, i64, i64, i64)] = &[
                ("unverified", "Unverified", "#9E9E9E", 0, 0, 0, 0, "unverified", 0,   280,   5,   4),
                ("verified",   "Verified",   "#4FC3F7", 1, 0, 1, 1, "verified",   1,  1000,  25,  20),
                ("donor",      "Donor",      "#FFD54F", 2, 0, 1, 1, "verified",   2,  1000,  25,  20),
                ("mod",        "Moderator",  "#81C784", 3, 1, 1, 1, "mod",        3,  4000, 100, 100),
                ("admin",      "Admin",      "#E57373", 4, 1, 1, 1, "admin",      4, 10000, 500, 500),
            ];
            for (id, label, color, trust, stream, upload, voice, tier, sort, mc, mu, mk) in seeds {
                conn.execute(
                    "INSERT OR IGNORE INTO roles
                       (id,label,color,trust_level,built_in,can_stream,can_upload,can_voice,base_tier,sort_order,can_image_share,can_file_share,max_chars,max_upload_mb,max_uploads_kept)
                     VALUES (?1,?2,?3,?4,1,?5,?6,?7,?8,?9,1,1,?10,?11,?12)",
                    rusqlite::params![id, label, color, trust, stream, upload, voice, tier, sort, mc, mu, mk],
                )?;
            }
            info!("Migration: roles built-ins seeded (post-ALTER ordering)");
        }

        // ── v0.245.0 — banned_keys gains a display name ──
        // The modal Ban button deletes the user's registered_names rows
        // (so they vanish from the member list), which means /unban <name>
        // could no longer resolve a key — a ban became irreversible. Store
        // the display name at ban time so the server-settings "Banned
        // users" panel can list who's banned and offer a per-row Unban.
        // Idempotent ALTER for relays banned before this migration (their
        // rows just show an empty name until re-banned).
        if conn.prepare("SELECT name FROM banned_keys LIMIT 0").is_err() {
            conn.execute_batch(
                "ALTER TABLE banned_keys ADD COLUMN name TEXT NOT NULL DEFAULT '';"
            )?;
            info!("Migration: added name column to banned_keys");
        }

        // ── v0.201.0 — split max_upload_mb into per-role columns ──
        // Operator: upload size should be variable per rank. New columns
        // default to a trust ladder (5/25/100/500 MB for unverified/
        // verified/mod/admin). On a relay that already has the v0.200
        // single-column row, we copy the existing max_upload_mb forward
        // into all four new columns so the operator's tuning isn't lost.
        // The old max_upload_mb column is RETAINED for backward compat
        // with v0.200 clients but is no longer the source of truth.
        if conn.prepare("SELECT max_upload_mb_unverified FROM server_settings LIMIT 0").is_err() {
            conn.execute_batch(
                "ALTER TABLE server_settings ADD COLUMN max_upload_mb_unverified INTEGER NOT NULL DEFAULT 5;
                 ALTER TABLE server_settings ADD COLUMN max_upload_mb_verified   INTEGER NOT NULL DEFAULT 25;
                 ALTER TABLE server_settings ADD COLUMN max_upload_mb_mod        INTEGER NOT NULL DEFAULT 100;
                 ALTER TABLE server_settings ADD COLUMN max_upload_mb_admin      INTEGER NOT NULL DEFAULT 500;"
            )?;
            // Data migration: if the relay was on v0.200 with a customized
            // max_upload_mb (anything other than the default 25), forward
            // that value into all four per-role columns so the operator's
            // tuning is preserved. Pure defaults (25) stay defaulted.
            let prev: i64 = conn.query_row(
                "SELECT max_upload_mb FROM server_settings WHERE id = 1",
                [],
                |row| row.get(0),
            ).unwrap_or(25);
            if prev != 25 {
                conn.execute(
                    "UPDATE server_settings SET
                        max_upload_mb_unverified = ?1,
                        max_upload_mb_verified   = ?1,
                        max_upload_mb_mod        = ?1,
                        max_upload_mb_admin      = ?1
                     WHERE id = 1",
                    rusqlite::params![prev],
                )?;
                info!(
                    "Migration: forwarded v0.200 max_upload_mb={} into all 4 per-role columns",
                    prev
                );
            }
            info!("Migration: split max_upload_mb into per-role columns (server_settings)");
        }

        // Follows table (social system).
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS follows (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                follower_key TEXT NOT NULL,
                followed_key TEXT NOT NULL,
                created_at  TEXT NOT NULL,
                UNIQUE(follower_key, followed_key)
            );

            CREATE INDEX IF NOT EXISTS idx_follows_follower
                ON follows(follower_key);
            CREATE INDEX IF NOT EXISTS idx_follows_followed
                ON follows(followed_key);"
        )?;

        // Groups tables (foundation).
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS groups (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                creator_key TEXT NOT NULL,
                created_at  TEXT NOT NULL,
                invite_code TEXT UNIQUE
            );

            CREATE TABLE IF NOT EXISTS group_members (
                group_id    TEXT NOT NULL,
                member_key  TEXT NOT NULL,
                role        TEXT NOT NULL DEFAULT 'member',
                joined_at   TEXT NOT NULL,
                PRIMARY KEY (group_id, member_key)
            );

            CREATE INDEX IF NOT EXISTS idx_group_members_key
                ON group_members(member_key);

            CREATE TABLE IF NOT EXISTS group_messages (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                group_id    TEXT NOT NULL,
                from_key    TEXT NOT NULL,
                from_name   TEXT NOT NULL DEFAULT '',
                content     TEXT NOT NULL,
                timestamp   INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_group_messages_group
                ON group_messages(group_id, timestamp);"
        )?;

        // Friend codes table (out-of-band friend discovery).
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS friend_codes (
                code TEXT PRIMARY KEY,
                public_key TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                uses_remaining INTEGER NOT NULL DEFAULT 1
            );

            CREATE INDEX IF NOT EXISTS idx_friend_codes_key
                ON friend_codes(public_key);"
        )?;

        // Marketplace listings table.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS marketplace_listings (
                id TEXT PRIMARY KEY,
                seller_key TEXT NOT NULL,
                seller_name TEXT,
                title TEXT NOT NULL,
                description TEXT,
                category TEXT NOT NULL,
                condition TEXT,
                price TEXT,
                payment_methods TEXT,
                location TEXT,
                images TEXT,
                status TEXT DEFAULT 'active',
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_marketplace_seller
                ON marketplace_listings(seller_key);
            CREATE INDEX IF NOT EXISTS idx_marketplace_status
                ON marketplace_listings(status);
            CREATE INDEX IF NOT EXISTS idx_marketplace_category
                ON marketplace_listings(category);

            CREATE TABLE IF NOT EXISTS listing_images (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                listing_id TEXT NOT NULL,
                url TEXT NOT NULL,
                position INTEGER DEFAULT 0,
                created_at TEXT NOT NULL,
                FOREIGN KEY (listing_id) REFERENCES marketplace_listings(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_listing_images_listing
                ON listing_images(listing_id, position);"
        )?;

        // FTS5 full-text search over marketplace listings.
        // Falls back silently if FTS5 is not compiled into this SQLite build.
        let marketplace_fts_ok = conn.execute_batch("
            CREATE VIRTUAL TABLE IF NOT EXISTS marketplace_fts
            USING fts5(listing_id UNINDEXED, title, description, category);
        ");

        if marketplace_fts_ok.is_ok() {
            info!("Marketplace FTS5 search index ready");
        } else {
            info!("Marketplace FTS5 not available — falling back to LIKE search");
        }

        // Listing reviews and seller aggregate ratings.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS listing_reviews (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                listing_id TEXT NOT NULL,
                reviewer_key TEXT NOT NULL,
                reviewer_name TEXT,
                rating INTEGER NOT NULL CHECK(rating >= 1 AND rating <= 5),
                comment TEXT DEFAULT '',
                created_at TEXT NOT NULL,
                FOREIGN KEY (listing_id) REFERENCES marketplace_listings(id) ON DELETE CASCADE,
                UNIQUE(listing_id, reviewer_key)
            );

            CREATE INDEX IF NOT EXISTS idx_reviews_listing
                ON listing_reviews(listing_id);
            CREATE INDEX IF NOT EXISTS idx_reviews_reviewer
                ON listing_reviews(reviewer_key);

            CREATE TABLE IF NOT EXISTS seller_ratings (
                seller_key TEXT PRIMARY KEY,
                avg_rating REAL DEFAULT 0,
                review_count INTEGER DEFAULT 0
            );"
        )?;

        // Assets table for the Asset Library.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS assets (
                id          TEXT PRIMARY KEY,
                owner_key   TEXT NOT NULL,
                filename    TEXT NOT NULL,
                file_type   TEXT NOT NULL,
                category    TEXT NOT NULL,
                tags        TEXT DEFAULT '[]',
                size_bytes  INTEGER NOT NULL DEFAULT 0,
                url         TEXT NOT NULL,
                description TEXT DEFAULT '',
                uploaded_at TEXT DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_assets_owner ON assets(owner_key);
            CREATE INDEX IF NOT EXISTS idx_assets_category ON assets(category);
            CREATE INDEX IF NOT EXISTS idx_assets_file_type ON assets(file_type);"
        )?;

        // Streams tables for live streaming history and chat.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS streams (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                streamer_key TEXT NOT NULL,
                title        TEXT NOT NULL DEFAULT '',
                category     TEXT NOT NULL DEFAULT '',
                started_at   INTEGER NOT NULL,
                ended_at     INTEGER,
                viewer_peak  INTEGER NOT NULL DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_streams_started
                ON streams(started_at);

            CREATE TABLE IF NOT EXISTS stream_chat (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                stream_id  INTEGER NOT NULL,
                content    TEXT NOT NULL,
                from_name  TEXT NOT NULL DEFAULT '',
                source     TEXT NOT NULL DEFAULT 'humanity',
                timestamp  INTEGER NOT NULL,
                FOREIGN KEY (stream_id) REFERENCES streams(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_stream_chat_stream
                ON stream_chat(stream_id, timestamp);"
        )?;

        // Skill DNA tables
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS user_skills (
                user_key    TEXT NOT NULL,
                skill_id    TEXT NOT NULL,
                reality_xp  REAL NOT NULL DEFAULT 0,
                fantasy_xp  REAL NOT NULL DEFAULT 0,
                level       INTEGER NOT NULL DEFAULT 0,
                updated_at  INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (user_key, skill_id)
            );

            CREATE INDEX IF NOT EXISTS idx_user_skills_skill
                ON user_skills(skill_id, level);

            CREATE TABLE IF NOT EXISTS skill_verifications (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                skill_id    TEXT NOT NULL,
                from_key    TEXT NOT NULL,
                to_key      TEXT NOT NULL,
                note        TEXT NOT NULL DEFAULT '',
                created_at  INTEGER NOT NULL DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_skill_verifications_to
                ON skill_verifications(to_key, skill_id);"
        )?;

        // Push notification subscriptions (WebPush API).
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS push_subscriptions (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                public_key  TEXT NOT NULL,
                endpoint    TEXT NOT NULL UNIQUE,
                p256dh      TEXT NOT NULL,
                auth        TEXT NOT NULL,
                created_at  INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_push_subs_key
                ON push_subscriptions(public_key);"
        )?;

        // Notification preferences per user (DM, mentions, tasks, DND schedule).
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS notification_prefs (
                public_key        TEXT PRIMARY KEY,
                dm_enabled        INTEGER NOT NULL DEFAULT 1,
                mentions_enabled  INTEGER NOT NULL DEFAULT 1,
                tasks_enabled     INTEGER NOT NULL DEFAULT 1,
                dnd_start         TEXT DEFAULT NULL,
                dnd_end           TEXT DEFAULT NULL
            );"
        )?;

        // Peer-to-peer trades with escrow.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS trades (
                id                TEXT PRIMARY KEY,
                initiator_key     TEXT NOT NULL,
                recipient_key     TEXT NOT NULL,
                status            TEXT NOT NULL DEFAULT 'pending',
                initiator_items   TEXT NOT NULL DEFAULT '[]',
                recipient_items   TEXT NOT NULL DEFAULT '[]',
                initiator_confirmed INTEGER DEFAULT 0,
                recipient_confirmed INTEGER DEFAULT 0,
                created_at        INTEGER NOT NULL,
                completed_at      INTEGER,
                message           TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_trades_initiator ON trades(initiator_key);
            CREATE INDEX IF NOT EXISTS idx_trades_recipient ON trades(recipient_key);
            CREATE INDEX IF NOT EXISTS idx_trades_status ON trades(status);"
        )?;

        // Order-book trading (sell orders with partial fills).
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS trade_orders (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                seller_key      TEXT NOT NULL,
                item_type       TEXT NOT NULL,
                item_id         TEXT NOT NULL DEFAULT '',
                quantity         INTEGER NOT NULL,
                remaining_qty   INTEGER NOT NULL,
                price_per_unit  REAL NOT NULL,
                currency        TEXT NOT NULL DEFAULT 'credits',
                status          TEXT NOT NULL DEFAULT 'open',
                created_at      INTEGER NOT NULL,
                filled_at       INTEGER
            );

            CREATE INDEX IF NOT EXISTS idx_trade_orders_item ON trade_orders(item_type, status);
            CREATE INDEX IF NOT EXISTS idx_trade_orders_seller ON trade_orders(seller_key, status);

            CREATE TABLE IF NOT EXISTS trade_history (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                order_id        INTEGER NOT NULL,
                buyer_key       TEXT NOT NULL,
                seller_key      TEXT NOT NULL,
                item_type       TEXT NOT NULL,
                item_id         TEXT NOT NULL DEFAULT '',
                quantity         INTEGER NOT NULL,
                price_per_unit  REAL NOT NULL,
                total_price     REAL NOT NULL,
                timestamp       INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_trade_history_buyer ON trade_history(buyer_key);
            CREATE INDEX IF NOT EXISTS idx_trade_history_seller ON trade_history(seller_key);
            CREATE INDEX IF NOT EXISTS idx_trade_history_item ON trade_history(item_type);
            CREATE INDEX IF NOT EXISTS idx_trade_history_order ON trade_history(order_id);"
        )?;

        // Listing messages for buyer-seller marketplace conversations.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS listing_messages (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                listing_id  TEXT NOT NULL,
                sender_key  TEXT NOT NULL,
                sender_name TEXT,
                content     TEXT NOT NULL,
                timestamp   INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_listing_messages_listing
                ON listing_messages(listing_id, timestamp);"
        )?;

        conn.execute_batch("
            -- Key rotation: maps an old identity key to a new one.
            -- old_key is PRIMARY KEY so each identity can only rotate forward once per entry.
            CREATE TABLE IF NOT EXISTS key_rotations (
                old_key    TEXT PRIMARY KEY,
                new_key    TEXT NOT NULL,
                sig_by_old TEXT NOT NULL,
                sig_by_new TEXT NOT NULL,
                rotated_at INTEGER NOT NULL
            );

            -- Encrypted vault blobs for cross-device sync.
            -- The blob is already AES-256-GCM encrypted by the client — we store it opaquely.
            CREATE TABLE IF NOT EXISTS vault_blobs (
                public_key TEXT PRIMARY KEY,
                blob       TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            );

            -- System profiles for hardware/OS context (not sensitive, stored as JSON).
            CREATE TABLE IF NOT EXISTS system_profiles (
                public_key TEXT PRIMARY KEY,
                profile    TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            );"
        )?;

        // Projects table for grouping tasks.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS projects (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                description TEXT DEFAULT '',
                owner_key   TEXT NOT NULL,
                visibility  TEXT DEFAULT 'public',
                color       TEXT DEFAULT '#4488ff',
                icon        TEXT DEFAULT '📋',
                created_at  TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_projects_owner ON projects(owner_key);

            -- Seed the default project so the system works immediately.
            INSERT OR IGNORE INTO projects (id, name, description, owner_key, visibility, created_at)
            VALUES ('default', 'General', 'Default project', 'system', 'public', datetime('now'));"
        )?;

        // Migration: add project column to project_tasks if missing.
        if conn.prepare("SELECT project FROM project_tasks LIMIT 0").is_err() {
            conn.execute_batch(
                "ALTER TABLE project_tasks ADD COLUMN project TEXT DEFAULT 'default';"
            )?;
            info!("Migration: added project column to project_tasks");
        }
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_project_tasks_project ON project_tasks(project);"
        )?;

        // Signed profiles: cryptographically signed, replicated across servers.
        // No home server — the signature is the authority. Latest timestamp wins.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS signed_profiles (
                public_key  TEXT PRIMARY KEY,
                name        TEXT NOT NULL DEFAULT '',
                bio         TEXT NOT NULL DEFAULT '',
                avatar_url  TEXT NOT NULL DEFAULT '',
                banner_url  TEXT NOT NULL DEFAULT '',
                socials     TEXT NOT NULL DEFAULT '{}',
                pronouns    TEXT NOT NULL DEFAULT '',
                location    TEXT NOT NULL DEFAULT '',
                website     TEXT NOT NULL DEFAULT '',
                timestamp   INTEGER NOT NULL DEFAULT 0,
                signature   TEXT NOT NULL DEFAULT ''
            );

            CREATE INDEX IF NOT EXISTS idx_signed_profiles_timestamp
                ON signed_profiles(timestamp);"
        )?;

        // Phase 0 substrate: generic post-quantum signed objects.
        // Every higher-level domain (signed_profiles, vouches, VCs, governance proposals,
        // recovery shares, etc.) is a projection of this table.
        // Format: see docs/network/object_format.md and src/relay/core/object.rs
        // Crypto: ML-DSA-65 (Dilithium3) — 1952-byte pubkey, 3309-byte signature.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS signed_objects (
                object_id              TEXT PRIMARY KEY,
                protocol_version       INTEGER NOT NULL,
                object_type            TEXT NOT NULL,
                space_id               TEXT,
                channel_id             TEXT,
                author_fp              TEXT NOT NULL,
                author_pubkey          BLOB NOT NULL,
                created_at             INTEGER,
                payload_schema_version INTEGER NOT NULL,
                payload_encoding       TEXT NOT NULL,
                payload                BLOB NOT NULL,
                signature              BLOB NOT NULL,
                references_json        TEXT NOT NULL DEFAULT '[]',
                source_server          TEXT,
                received_at            INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_signed_objects_type_space
                ON signed_objects(object_type, space_id);

            CREATE INDEX IF NOT EXISTS idx_signed_objects_author_fp
                ON signed_objects(author_fp);

            CREATE INDEX IF NOT EXISTS idx_signed_objects_received_at
                ON signed_objects(received_at);

            -- P2P groups (docs/design/p2p-groups.md, Phase 1). PROJECTION of the
            -- group_v1 + group_member_v1 signed objects — a fast-read cache; the
            -- signed objects remain the authority and replicate peer-to-peer.
            -- group_id = the group_v1 object's object_id (self-certifying).
            CREATE TABLE IF NOT EXISTS p2p_groups (
                group_id        TEXT PRIMARY KEY,
                name            TEXT NOT NULL,
                creator_fp      TEXT NOT NULL,
                creator_pubkey  BLOB NOT NULL,
                created_at      INTEGER,
                -- 1 once the creator publishes a group_disband_v1 — the group
                -- then disappears from every member's list (the disband object
                -- is the durable, P2P-replicable tombstone).
                disbanded       INTEGER NOT NULL DEFAULT 0
            );

            -- The fold of the append-only membership log: who is currently in.
            CREATE TABLE IF NOT EXISTS p2p_group_roster (
                group_id      TEXT NOT NULL,
                member_fp     TEXT NOT NULL,
                member_pubkey BLOB NOT NULL,
                active        INTEGER NOT NULL DEFAULT 1,
                updated_at    INTEGER NOT NULL,
                PRIMARY KEY (group_id, member_fp)
            );
            CREATE INDEX IF NOT EXISTS idx_p2p_roster_member
                ON p2p_group_roster(member_fp);

            -- Creator-signed invite capabilities (projection of group_invite_v1).
            -- secret_hash = BLAKE3(invite secret); a joiner proves they hold the
            -- out-of-band ticket by revealing the secret in their group_join_v1.
            -- Lets members join WITHOUT the creator being online (the creator's
            -- signature on the invite is the standing authorization).
            CREATE TABLE IF NOT EXISTS p2p_group_invites (
                invite_id    TEXT PRIMARY KEY,
                group_id     TEXT NOT NULL,
                secret_hash  BLOB NOT NULL,
                expires_at   INTEGER NOT NULL,
                created_at   INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_p2p_invites_group
                ON p2p_group_invites(group_id);

            -- Phase 2: per-epoch group key objects (group_epoch_key_v1). The
            -- relay only indexes WHICH object holds epoch N's sealed keys — it
            -- never sees the key (each member's copy is ML-KEM-sealed inside the
            -- object payload). Members fetch the object + decrypt their entry.
            CREATE TABLE IF NOT EXISTS p2p_group_epochs (
                group_id    TEXT NOT NULL,
                epoch       INTEGER NOT NULL,
                object_id   TEXT NOT NULL,
                created_at  INTEGER,
                PRIMARY KEY (group_id, epoch)
            );

            -- Phase 2: encrypted group message log (group_msg_v1). The relay
            -- stores/serves the OPAQUE ciphertext (it cannot decrypt); only
            -- active members (roster) are projected here. The payload lives in
            -- signed_objects keyed by object_id.
            CREATE TABLE IF NOT EXISTS p2p_group_messages (
                object_id   TEXT PRIMARY KEY,
                group_id    TEXT NOT NULL,
                author_fp   TEXT NOT NULL,
                epoch       INTEGER NOT NULL,
                created_at  INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_p2p_messages_group
                ON p2p_group_messages(group_id, created_at);

            -- Phase 1 PR 2: Verifiable Credentials fast-lookup index.
            -- Auto-populated when a known VC schema is stored. The credential itself
            -- (signed authority) lives in signed_objects keyed by vc_object_id.
            CREATE TABLE IF NOT EXISTS vc_index (
                vc_object_id          TEXT PRIMARY KEY,
                issuer_did            TEXT NOT NULL,
                subject_did           TEXT NOT NULL,
                schema_id             TEXT NOT NULL,
                issued_at             INTEGER NOT NULL,
                expires_at            INTEGER,
                revoked_by_object_id  TEXT,
                withdrawn             INTEGER NOT NULL DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_vc_subject ON vc_index(subject_did);
            CREATE INDEX IF NOT EXISTS idx_vc_issuer  ON vc_index(issuer_did);
            CREATE INDEX IF NOT EXISTS idx_vc_schema  ON vc_index(schema_id);

            -- Phase 2 PR 1: Multi-layer trust score cache.
            CREATE TABLE IF NOT EXISTS trust_scores (
                did                TEXT PRIMARY KEY,
                total              REAL NOT NULL,
                sub_scores_json    TEXT NOT NULL,
                inputs_json        TEXT NOT NULL,
                weights_version    INTEGER NOT NULL,
                computed_at        INTEGER NOT NULL
            );

            -- Phase 5 PR 1: Governance proposals fast-lookup index.
            CREATE TABLE IF NOT EXISTS proposals (
                proposal_object_id  TEXT PRIMARY KEY,
                proposer_did        TEXT NOT NULL,
                proposal_type       TEXT NOT NULL,
                scope               TEXT NOT NULL,
                space_id            TEXT,
                opens_at            INTEGER NOT NULL,
                closes_at           INTEGER NOT NULL,
                created_at          INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_proposals_scope ON proposals(scope);
            CREATE INDEX IF NOT EXISTS idx_proposals_type  ON proposals(proposal_type);
            CREATE INDEX IF NOT EXISTS idx_proposals_space ON proposals(space_id);

            -- Phase 5 PR 1: Vote records. UNIQUE on (proposal, voter) prevents
            -- double-voting at the index level even if multiple distinct vote
            -- objects from the same voter survive deduplication.
            CREATE TABLE IF NOT EXISTS votes (
                vote_object_id      TEXT PRIMARY KEY,
                proposal_object_id  TEXT NOT NULL,
                voter_did           TEXT NOT NULL,
                choice              TEXT NOT NULL,
                weight_at_vote      REAL NOT NULL,
                cast_at             INTEGER NOT NULL,
                UNIQUE(proposal_object_id, voter_did)
            );
            CREATE INDEX IF NOT EXISTS idx_votes_proposal ON votes(proposal_object_id);
            CREATE INDEX IF NOT EXISTS idx_votes_voter    ON votes(voter_did);

            -- Phase 8 PR 1: AI-as-citizen status. Tracks subject_class declarations
            -- and controlled_by_v1 operator bindings per DID. AI agents must have
            -- a non-NULL operator_did to interact (enforced in put_signed_object).
            CREATE TABLE IF NOT EXISTS ai_status (
                did             TEXT PRIMARY KEY,
                subject_class   TEXT NOT NULL,
                operator_did    TEXT,
                last_updated    INTEGER NOT NULL
            );

            -- Phase 4 PR 1: Social key recovery — opaque Shamir share index.
            -- The encrypted share ciphertext lives in signed_objects (payload);
            -- this table is just a fast-lookup index for the recovery flow.
            CREATE TABLE IF NOT EXISTS recovery_shares (
                share_object_id  TEXT PRIMARY KEY,
                holder_did       TEXT NOT NULL,
                guardian_did     TEXT NOT NULL,
                threshold        INTEGER NOT NULL,
                total_shares     INTEGER NOT NULL,
                created_at       INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_recovery_holder   ON recovery_shares(holder_did);
            CREATE INDEX IF NOT EXISTS idx_recovery_guardian ON recovery_shares(guardian_did);

            -- Phase 4 PR 2: Recovery request + approval tracking. Holder publishes
            -- a recovery_request_v1 signed by their NEW key; guardians publish
            -- recovery_approval_v1 referencing the request. When approvals_count
            -- reaches threshold_required, status flips to 'ready' and the holder's
            -- client can reassemble shares via Shamir and publish a key_rotation_v1.
            CREATE TABLE IF NOT EXISTS recovery_requests (
                request_object_id    TEXT PRIMARY KEY,
                holder_did           TEXT NOT NULL,
                new_pubkey           BLOB NOT NULL,
                threshold_required   INTEGER NOT NULL,
                approvals_count      INTEGER NOT NULL DEFAULT 0,
                status               TEXT NOT NULL DEFAULT 'open',
                created_at           INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_recovery_requests_holder ON recovery_requests(holder_did);
            CREATE INDEX IF NOT EXISTS idx_recovery_requests_status ON recovery_requests(status);

            CREATE TABLE IF NOT EXISTS recovery_approvals (
                approval_object_id   TEXT PRIMARY KEY,
                request_object_id    TEXT NOT NULL,
                guardian_did         TEXT NOT NULL,
                submitted_at         INTEGER NOT NULL,
                UNIQUE(request_object_id, guardian_did)
            );
            CREATE INDEX IF NOT EXISTS idx_recovery_approvals_request
                ON recovery_approvals(request_object_id);

            -- v0.116.0: multi-AI agent coordination — claim/heartbeat/release a scope.
            -- Lets multiple Claude Code sessions check in / out of specific scopes
            -- without trampling each other. agent_registry.ron declares canonical
            -- scopes; this table is the live runtime state.
            CREATE TABLE IF NOT EXISTS agent_sessions (
                scope_id            TEXT PRIMARY KEY,
                agent_id            TEXT NOT NULL,
                state               TEXT NOT NULL DEFAULT 'working',
                last_state_notes    TEXT NOT NULL DEFAULT '',
                claimed_at          INTEGER NOT NULL,
                last_heartbeat      INTEGER NOT NULL,
                completion_estimate REAL
            );
            CREATE INDEX IF NOT EXISTS idx_agent_sessions_heartbeat
                ON agent_sessions(last_heartbeat);

            -- Phase 3 PR 2: per-observer per-issuer continuous trust matrix.
            -- Each server tracks how much it trusts each issuer DID it has seen.
            -- Disputes drop trust; valid VCs raise it. Caps in [0, 1].
            CREATE TABLE IF NOT EXISTS issuer_trust (
                observer_server  TEXT NOT NULL,
                issuer_did       TEXT NOT NULL,
                trust            REAL NOT NULL,
                good_count       INTEGER NOT NULL DEFAULT 0,
                bad_count        INTEGER NOT NULL DEFAULT 0,
                last_event_at    INTEGER NOT NULL,
                PRIMARY KEY (observer_server, issuer_did)
            );
            CREATE INDEX IF NOT EXISTS idx_issuer_trust_did ON issuer_trust(issuer_did);"
        )?;

        // Migration: add origin_server column to messages for federated message persistence.
        if conn.prepare("SELECT origin_server FROM messages LIMIT 0").is_err() {
            let _ = conn.execute(
                "ALTER TABLE messages ADD COLUMN origin_server TEXT DEFAULT NULL",
                [],
            );
            info!("Migration: added origin_server column to messages");
        }

        // Migration (v0.301.0): add disbanded flag to p2p_groups so a creator
        // can tear a group down (group_disband_v1). Existing live DBs created
        // the table pre-v0.301 without the column; fresh DBs get it from the
        // CREATE above. DEFAULT 0 = every existing group stays live.
        if conn.prepare("SELECT disbanded FROM p2p_groups LIMIT 0").is_err() {
            let _ = conn.execute(
                "ALTER TABLE p2p_groups ADD COLUMN disbanded INTEGER NOT NULL DEFAULT 0",
                [],
            );
            info!("Migration: added disbanded column to p2p_groups");
        }

        // Server members table (membership tiers: member, contributor, mod, admin).
        // Guests have no row — they're just connected WebSocket peers.
        // Owner is stored in server-config.json, not this table.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS server_members (
                public_key TEXT PRIMARY KEY,
                name       TEXT,
                role       TEXT NOT NULL DEFAULT 'member',
                joined_at  TEXT NOT NULL,
                last_seen  TEXT
            );"
        )?;

        // FTS5 full-text search over chat messages.
        // Uses a content table (content=messages) so we don't duplicate data.
        // Triggers keep the index in sync with every insert/update/delete.
        // Falls back silently if FTS5 is not compiled into this SQLite build.
        //
        // Migration: drop old 2-column FTS table if it exists (was: content, from_name).
        // The new table adds channel_id for column-scoped search.
        let has_fts_channel: bool = conn
            .prepare("SELECT channel_id FROM messages_fts LIMIT 0")
            .is_ok();
        if !has_fts_channel {
            let _ = conn.execute_batch("
                DROP TRIGGER IF EXISTS messages_fts_ai;
                DROP TRIGGER IF EXISTS messages_fts_ad;
                DROP TRIGGER IF EXISTS messages_fts_au;
                DROP TABLE IF EXISTS messages_fts;
            ");
            info!("Migration: rebuilding FTS5 index with channel_id column");
        }

        let fts_ok = conn.execute_batch("
            CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts
            USING fts5(content, from_name, channel_id, content='messages', content_rowid='id');

            -- Keep FTS index up to date automatically
            CREATE TRIGGER IF NOT EXISTS messages_fts_ai AFTER INSERT ON messages BEGIN
                INSERT INTO messages_fts(rowid, content, from_name, channel_id)
                VALUES (new.id, COALESCE(new.content,''), COALESCE(new.from_name,''), COALESCE(new.channel_id,'general'));
            END;
            CREATE TRIGGER IF NOT EXISTS messages_fts_ad AFTER DELETE ON messages BEGIN
                INSERT INTO messages_fts(messages_fts, rowid, content, from_name, channel_id)
                VALUES ('delete', old.id, COALESCE(old.content,''), COALESCE(old.from_name,''), COALESCE(old.channel_id,'general'));
            END;
            CREATE TRIGGER IF NOT EXISTS messages_fts_au AFTER UPDATE ON messages BEGIN
                INSERT INTO messages_fts(messages_fts, rowid, content, from_name, channel_id)
                VALUES ('delete', old.id, COALESCE(old.content,''), COALESCE(old.from_name,''), COALESCE(old.channel_id,'general'));
                INSERT INTO messages_fts(rowid, content, from_name, channel_id)
                VALUES (new.id, COALESCE(new.content,''), COALESCE(new.from_name,''), COALESCE(new.channel_id,'general'));
            END;
        ");

        if fts_ok.is_ok() {
            // Populate FTS for any rows that pre-date the virtual table.
            // 'rebuild' is idempotent with content tables and only re-reads missing rows.
            let _ = conn.execute_batch(
                "INSERT INTO messages_fts(messages_fts) VALUES ('rebuild');"
            );
            info!("FTS5 search index ready");
        } else {
            info!("FTS5 not available — falling back to LIKE search");
        }

        // Guilds tables.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS guilds (
                id           TEXT PRIMARY KEY,
                name         TEXT NOT NULL,
                description  TEXT NOT NULL DEFAULT '',
                owner_key    TEXT NOT NULL,
                icon         TEXT NOT NULL DEFAULT '',
                color        TEXT NOT NULL DEFAULT '#4488ff',
                created_at   TEXT NOT NULL,
                member_count INTEGER NOT NULL DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_guilds_owner ON guilds(owner_key);

            CREATE TABLE IF NOT EXISTS guild_members (
                guild_id    TEXT NOT NULL,
                public_key  TEXT NOT NULL,
                role        TEXT NOT NULL DEFAULT 'member',
                joined_at   TEXT NOT NULL,
                PRIMARY KEY (guild_id, public_key)
            );

            CREATE INDEX IF NOT EXISTS idx_guild_members_key ON guild_members(public_key);

            CREATE TABLE IF NOT EXISTS guild_invites (
                id              TEXT PRIMARY KEY,
                guild_id        TEXT NOT NULL,
                created_by      TEXT NOT NULL,
                code            TEXT NOT NULL UNIQUE,
                uses_remaining  INTEGER NOT NULL DEFAULT 1,
                expires_at      INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_guild_invites_code ON guild_invites(code);
            CREATE INDEX IF NOT EXISTS idx_guild_invites_guild ON guild_invites(guild_id);"
        )?;

        // Reputation tables.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS reputation (
                public_key  TEXT PRIMARY KEY,
                score       INTEGER NOT NULL DEFAULT 0,
                level       INTEGER NOT NULL DEFAULT 0,
                updated_at  INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS reputation_events (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                public_key  TEXT NOT NULL,
                event_type  TEXT NOT NULL,
                points      INTEGER NOT NULL,
                reason      TEXT NOT NULL DEFAULT '',
                created_at  INTEGER NOT NULL,
                source_key  TEXT NOT NULL DEFAULT ''
            );

            CREATE INDEX IF NOT EXISTS idx_reputation_events_key
                ON reputation_events(public_key, created_at);
            CREATE INDEX IF NOT EXISTS idx_reputation_score
                ON reputation(score DESC);"
        )?;

        // Bug reports table.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS bug_reports (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                title           TEXT NOT NULL,
                description     TEXT NOT NULL,
                steps           TEXT NOT NULL DEFAULT '',
                expected        TEXT NOT NULL DEFAULT '',
                actual          TEXT NOT NULL DEFAULT '',
                severity        TEXT NOT NULL DEFAULT 'medium',
                category        TEXT NOT NULL DEFAULT 'other',
                reporter_key    TEXT NOT NULL,
                reporter_name   TEXT NOT NULL DEFAULT '',
                browser_info    TEXT NOT NULL DEFAULT '',
                page_url        TEXT NOT NULL DEFAULT '',
                version         TEXT NOT NULL DEFAULT '',
                status          TEXT NOT NULL DEFAULT 'open',
                votes           INTEGER NOT NULL DEFAULT 0,
                created_at      INTEGER NOT NULL,
                updated_at      INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_bug_reports_status ON bug_reports(status);
            CREATE INDEX IF NOT EXISTS idx_bug_reports_severity ON bug_reports(severity);
            CREATE INDEX IF NOT EXISTS idx_bug_reports_category ON bug_reports(category);
            CREATE INDEX IF NOT EXISTS idx_bug_reports_reporter ON bug_reports(reporter_key);

            CREATE TABLE IF NOT EXISTS bug_votes (
                bug_id      INTEGER NOT NULL,
                voter_key   TEXT NOT NULL,
                voted_at    INTEGER NOT NULL,
                PRIMARY KEY (bug_id, voter_key),
                FOREIGN KEY (bug_id) REFERENCES bug_reports(id) ON DELETE CASCADE
            );"
        )?;

        // Build the read-only connection pool NOW — after every CREATE
        // TABLE / migration above has run on the writer `conn`, so a pooled
        // reader never observes a schema-less database. The pool opens the
        // SAME file read-only with matching WAL/foreign_keys/busy_timeout
        // pragmas (see `pool::build_read_pool`). All current call sites still
        // use the writer via `with_conn`; read-heavy paths can adopt
        // `with_read_conn` incrementally for concurrency.
        let read_pool = pool::build_read_pool(path)?;

        info!("Database opened: {}", path.display());
        Ok(Self { conn: Mutex::new(conn), read_pool })
    }
}

// Connection-pool plumbing for concurrent reads (R3). Not a domain module —
// it provides the read-only `r2d2` pool that backs `Storage::with_read_conn`.
mod pool;

// Domain method modules — each has its own impl Storage block.
// Rust supports splitting impl blocks across files via the module system.
mod assets;
mod board;
mod channels;
mod dms;
mod key_rotation;
mod marketplace;
mod messages;
mod misc;
mod pins;
mod profile;
mod projects;
mod push;
mod reactions;
mod skill_dna;
mod social;
mod streams;
mod system;
mod uploads;
mod reviews;
mod members;
mod server_settings;
pub use server_settings::ServerSettings;
mod roles;
pub use roles::RoleDef;
pub use channels::BannedUser;
pub use channels::MutedUser;
mod agent_sessions;
mod ai_status;
mod credentials;
mod dids;
mod governance;
mod issuer_trust;
mod recovery;
mod signed_objects;
mod groups_p2p;
mod signed_profiles;
mod trust_score;
mod notification_prefs;
mod trading;
mod vault_sync;
mod civilization;
pub mod files;
mod bugs;
mod guilds;
mod reputation;
mod game_persistence;

pub use civilization::CivilizationStats;
pub use guilds::{GuildRecord, GuildMemberRecord, GuildInviteRecord};
pub use marketplace::ListingMessageRecord;
pub use notification_prefs::NotifPrefs;
pub use bugs::BugReport;
pub use reputation::{ReputationRecord, ReputationEventRecord};
pub use trading::TradeRecord;
// Durable game-world snapshot + per-player progress (relay game-state
// persistence). See game_persistence.rs for the rationale + schema.
pub use game_persistence::{GameWorldSnapshot, PlayerProgress};

#[cfg(test)]
mod resilient_open_tests {
    //! Coverage for open_resilient() corruption detection + backup
    //! restore (TIER 1 #3, post-2026-05-21). The healthy path must be a
    //! no-op wrapper around open(); the corrupt path must restore the
    //! newest HEALTHY backup; with no healthy backup it must FAIL LOUD
    //! (Err) rather than silently wipe or run corrupt.
    use super::*;

    fn tmp_dir(tag: &str) -> std::path::PathBuf {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("hum_resilient_{tag}_{pid}_{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Create a real, healthy relay DB at `path` with one known message
    /// so we can later assert which DB we ended up with.
    fn make_healthy_db(path: &Path, marker_ts: u64) {
        let s = Storage::open(path).expect("open healthy");
        s.with_conn(|c| {
            c.execute(
                "INSERT INTO messages (msg_type, from_key, from_name, content, timestamp, raw_json, channel_id)
                 VALUES ('chat','pk','name','hello', ?1, '{}', 'general')",
                rusqlite::params![marker_ts as i64],
            )
        }).unwrap();
        // Drop so WAL is checkpointed + file is closed before we copy it.
        drop(s);
    }

    fn marker_count(path: &Path, ts: u64) -> i64 {
        let s = Storage::open(path).unwrap();
        s.with_conn(|c| c.query_row(
            "SELECT COUNT(*) FROM messages WHERE timestamp = ?1",
            rusqlite::params![ts as i64],
            |r| r.get::<_, i64>(0),
        )).unwrap()
    }

    #[test]
    fn healthy_db_opens_normally() {
        let dir = tmp_dir("healthy");
        let db = dir.join("relay.db");
        let backups = dir.join("backups");
        std::fs::create_dir_all(&backups).unwrap();
        make_healthy_db(&db, 111);

        let s = Storage::open_resilient(&db, &backups).expect("healthy open_resilient");
        // The marker row is intact -> we opened the real DB, not a fresh one.
        let n = s.with_conn(|c| c.query_row(
            "SELECT COUNT(*) FROM messages WHERE timestamp = 111",
            [], |r| r.get::<_, i64>(0),
        )).unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn corrupt_db_restores_newest_healthy_backup() {
        let dir = tmp_dir("restore");
        let db = dir.join("relay.db");
        let backups = dir.join("backups");
        std::fs::create_dir_all(&backups).unwrap();

        // A healthy backup carrying marker 222.
        let backup = backups.join("relay-20260101-000000.db");
        make_healthy_db(&backup, 222);

        // The live DB is garbage (not a SQLite file at all).
        std::fs::write(&db, b"this is not a database, it is corrupt garbage").unwrap();

        let s = Storage::open_resilient(&db, &backups).expect("should recover from backup");
        // We restored the backup -> marker 222 present.
        let n = s.with_conn(|c| c.query_row(
            "SELECT COUNT(*) FROM messages WHERE timestamp = 222",
            [], |r| r.get::<_, i64>(0),
        )).unwrap();
        assert_eq!(n, 1);
        // The corrupt original was quarantined, not deleted.
        let quarantined = std::fs::read_dir(&dir).unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().contains("relay.db.corrupt-"));
        assert!(quarantined, "expected a quarantined relay.db.corrupt-* file");
    }

    #[test]
    fn corrupt_db_no_backup_fails_loud_without_wiping() {
        let dir = tmp_dir("noback");
        let db = dir.join("relay.db");
        let backups = dir.join("backups");
        std::fs::create_dir_all(&backups).unwrap();

        // Corrupt live DB, EMPTY backups dir.
        std::fs::write(&db, b"corrupt, and no backups exist").unwrap();
        let before = std::fs::read(&db).unwrap();

        let result = Storage::open_resilient(&db, &backups);
        assert!(result.is_err(), "must fail loud when no healthy backup exists");
        // CRITICAL: the corrupt file must be LEFT IN PLACE (not quarantined
        // into a fresh-empty-schema situation, not wiped). Operator decides.
        let after = std::fs::read(&db).unwrap();
        assert_eq!(before, after, "corrupt live DB must be untouched on failed recovery");
    }

    #[test]
    fn skips_corrupt_backup_for_older_healthy_one() {
        let dir = tmp_dir("skip");
        let db = dir.join("relay.db");
        let backups = dir.join("backups");
        std::fs::create_dir_all(&backups).unwrap();

        // Older healthy backup (marker 333).
        let good = backups.join("relay-20260101-000000.db");
        make_healthy_db(&good, 333);
        // Make sure the newer one has a later mtime by writing it second.
        std::thread::sleep(std::time::Duration::from_millis(20));
        // Newer but CORRUPT backup.
        let bad = backups.join("relay-20260102-000000.db");
        std::fs::write(&bad, b"newer but corrupt backup").unwrap();

        // Corrupt live DB.
        std::fs::write(&db, b"corrupt live").unwrap();

        let s = Storage::open_resilient(&db, &backups).expect("should fall back to older healthy backup");
        assert_eq!(marker_count(&db, 333), 1, "should have restored the older HEALTHY backup");
    }
}
