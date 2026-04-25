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
pub use signed_objects::{SignedObjectRecord, author_fingerprint, compute_object_id};

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
pub struct Storage {
    pub(crate) conn: Mutex<Connection>,
}

/// Shared timestamp helper used by multiple submodules.
pub(crate) fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

impl Storage {
    /// Execute a closure with a locked database connection.
    /// Panics if the mutex is poisoned (same as current unwrap behavior).
    pub(crate) fn with_conn<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&Connection) -> T,
    {
        let conn = self.conn.lock().unwrap();
        f(&conn)
    }

    /// Like `with_conn` but provides a mutable reference (needed for transactions).
    pub(crate) fn with_conn_mut<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&mut Connection) -> T,
    {
        let mut conn = self.conn.lock().unwrap();
        f(&mut conn)
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

            CREATE TABLE IF NOT EXISTS registered_names (
                name        TEXT NOT NULL COLLATE NOCASE,
                public_key  TEXT NOT NULL,
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
                banned_at   INTEGER NOT NULL
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

        // Migration: add ecdh_public column to registered_names for E2EE DMs.
        let has_ecdh: bool = conn
            .prepare("SELECT ecdh_public FROM registered_names LIMIT 0")
            .is_ok();
        if !has_ecdh {
            conn.execute_batch(
                "ALTER TABLE registered_names ADD COLUMN ecdh_public TEXT DEFAULT NULL;"
            )?;
            info!("Migration: added ecdh_public column to registered_names");
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
            );"
        )?;

        // Migration: add origin_server column to messages for federated message persistence.
        if conn.prepare("SELECT origin_server FROM messages LIMIT 0").is_err() {
            let _ = conn.execute(
                "ALTER TABLE messages ADD COLUMN origin_server TEXT DEFAULT NULL",
                [],
            );
            info!("Migration: added origin_server column to messages");
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

        info!("Database opened: {}", path.display());
        Ok(Self { conn: Mutex::new(conn) })
    }
}

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
mod ai_status;
mod credentials;
mod dids;
mod governance;
mod signed_objects;
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

pub use civilization::CivilizationStats;
pub use guilds::{GuildRecord, GuildMemberRecord, GuildInviteRecord};
pub use marketplace::ListingMessageRecord;
pub use notification_prefs::NotifPrefs;
pub use bugs::BugReport;
pub use reputation::{ReputationRecord, ReputationEventRecord};
pub use trading::TradeRecord;
