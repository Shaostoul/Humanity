//! Core relay logic: connection management and message routing.

use axum::extract::ws::{Message, WebSocket};
use ed25519_dalek::{Signature, VerifyingKey};
use futures::{SinkExt, StreamExt};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use tokio::sync::{broadcast, RwLock};
use tracing::info;

use crate::storage::Storage;
use web_push_native::jwt_simple::prelude::ES256KeyPair;

use crate::handlers::*;
use crate::handlers::game_state::GameWorld;
// Re-export start_federation_connections so main.rs can call relay::start_federation_connections.
pub use crate::handlers::start_federation_connections;

/// Allowed emoji reactions (whitelist).
const ALLOWED_REACTIONS: &[&str] = &["❤️", "😂", "👍", "👎", "🔥", "😮", "😢", "🎉"];

/// Maximum broadcast channel capacity.
const BROADCAST_CAPACITY: usize = 256;

/// Maximum concurrent WebSocket connections.
const MAX_CONNECTIONS: usize = 500;

/// Timeout for the initial identify message (seconds).
const IDENTIFY_TIMEOUT_SECS: u64 = 30;

/// Minimum interval between typing indicators per user (seconds).
const TYPING_RATE_LIMIT_SECS: u64 = 2;

/// Fibonacci delay sequence in seconds (capped at 21s).
pub const FIB_DELAYS: [u64; 8] = [1, 1, 2, 3, 5, 8, 13, 21];

/// Duration after which a new identity is no longer considered "new" (10 minutes).
pub const NEW_ACCOUNT_WINDOW_SECS: u64 = 600;

/// Whether registrations should stay open by default even when staff are offline.
/// Controlled by env REGISTRATION_DEFAULT_OPEN (default: true).
fn registration_default_open() -> bool {
    std::env::var("REGISTRATION_DEFAULT_OPEN")
        .ok()
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(true)
}

/// Flat rate limit for new accounts (seconds).
pub const NEW_ACCOUNT_DELAY_SECS: u64 = 5;

/// Per-key rate limit tracking state.
#[derive(Debug, Clone)]
pub struct RateLimitState {
    /// When the key was first seen (for new-account slow mode).
    pub first_seen: Instant,
    /// When the last message was sent.
    pub last_message_time: Instant,
    /// Current position in the Fibonacci delay sequence.
    pub fib_index: usize,
}

/// A connected peer, identified by their public key hex.
#[derive(Debug, Clone)]
pub struct Peer {
    pub public_key_hex: String,
    pub display_name: Option<String>,
    /// Per-session upload token (M-4: prevents impersonation on uploads).
    pub upload_token: Option<String>,
    /// ECDH P-256 public key (base64 raw) for E2E encrypted DMs.
    pub ecdh_public: Option<String>,
}

/// Maximum message history to keep in memory.
const MAX_HISTORY: usize = 500;

/// Webhook configuration for notifying external services of new messages.
#[derive(Debug, Clone)]
pub struct WebhookConfig {
    /// URL to POST to when a human sends a message.
    pub url: String,
    /// Optional bearer token for authentication.
    pub token: Option<String>,
}

/// Shared relay state.
pub struct RelayState {
    /// Connected peers by public key hex.
    pub peers: RwLock<HashMap<String, Peer>>,
    /// Broadcast channel for messages.
    pub broadcast_tx: broadcast::Sender<RelayMessage>,
    /// In-memory recent history (fast access for WebSocket clients).
    pub history: RwLock<Vec<RelayMessage>>,
    /// Persistent storage (SQLite).
    pub db: Storage,
    /// Server start time (for uptime reporting).
    pub start_time: std::time::Instant,
    /// Optional webhook for new-message notifications.
    pub webhook: Option<WebhookConfig>,
    /// HTTP client for webhook calls.
    pub http_client: reqwest::Client,
    /// Per-key rate limiting state (Fibonacci backoff).
    pub rate_limits: RwLock<HashMap<String, RateLimitState>>,
    /// Lockdown mode: when true, new name registrations are blocked.
    pub lockdown: RwLock<bool>,
    /// Whether the current lockdown was set automatically (vs manually).
    /// Only auto-unlock if lockdown was auto-set.
    pub auto_lockdown: RwLock<bool>,
    /// Keys that have been kicked/banned — their WebSocket loops check this
    /// and close the connection when they find themselves listed.
    pub kicked_keys: RwLock<HashSet<String>>,
    /// Active WebSocket connection count (for connection limiting).
    pub connection_count: AtomicUsize,
    /// Per-key last typing indicator time (for typing rate limiting).
    pub typing_timestamps: RwLock<HashMap<String, Instant>>,
    /// Upload token → public key mapping (M-4: per-session upload tokens).
    pub upload_tokens: RwLock<HashMap<String, String>>,
    /// Active voice rooms (room_id → VoiceRoom).
    pub voice_rooms: RwLock<HashMap<String, VoiceRoom>>,
    /// User status cache (name → (status, status_text)).
    pub user_statuses: RwLock<HashMap<String, (String, String)>>,
    /// Per-key last search time (rate limiting: 1 search per 2 seconds).
    pub last_search_times: std::sync::Mutex<HashMap<String, std::time::Instant>>,
    /// Active stream (only one at a time for MVP).
    pub active_stream: RwLock<Option<ActiveStream>>,
    /// Active federation connections (server_id → FederatedConnection).
    pub federation_connections: RwLock<HashMap<String, FederatedConnection>>,
    /// Rate limiter for federation message forwarding (server_id → last send times).
    pub federation_rate: std::sync::Mutex<HashMap<String, Vec<Instant>>>,
    /// VAPID keypair for WebPush notifications (P-256/ES256).
    pub vapid_key: Option<ES256KeyPair>,
    /// Server configuration loaded from data/server-config.json (funding, membership, etc.).
    pub server_config: serde_json::Value,
    /// Server-authoritative game world state.
    pub game_world: RwLock<GameWorld>,
}

impl RelayState {
    pub fn new(db: Storage) -> Self {
        // Read webhook config from environment.
        let webhook = std::env::var("WEBHOOK_URL").ok().map(|url| {
            WebhookConfig {
                url,
                token: std::env::var("WEBHOOK_TOKEN").ok(),
            }
        });

        if let Some(ref wh) = webhook {
            info!("Webhook configured: {}", wh.url);
        }

        // Load recent history from database.
        let history = db.load_recent_messages(MAX_HISTORY).unwrap_or_default();
        let history_count = history.len();
        if history_count > 0 {
            info!("Loaded {history_count} messages from database");
        }

        // L-4: Restore lockdown state from DB.
        let persisted_lockdown = db.get_state("lockdown")
            .ok()
            .flatten()
            .map(|v| v == "true")
            .unwrap_or(false);

        // Default-open policy: force-open registrations unless explicitly disabled.
        let default_open = registration_default_open();
        let effective_lockdown = if default_open {
            if persisted_lockdown {
                let _ = db.set_state("lockdown", "false");
                info!("Registration default-open policy active; overriding persisted lockdown=true to false");
            }
            false
        } else {
            persisted_lockdown
        };

        if effective_lockdown {
            info!("Restored lockdown state from database: locked");
        }

        // Load server config from data/server-config.json (or use defaults).
        let server_config = std::fs::read_to_string("data/server-config.json")
            .ok()
            .and_then(|contents| serde_json::from_str::<serde_json::Value>(&contents).ok())
            .unwrap_or_else(|| {
                info!("data/server-config.json not found or invalid, using defaults");
                serde_json::json!({
                    "server_name": "Humanity Relay",
                    "server_description": "",
                    "owner_key": "",
                    "funding": { "enabled": false }
                })
            });
        info!("Server config loaded: {}", server_config.get("server_name").and_then(|v| v.as_str()).unwrap_or("unknown"));

        let (broadcast_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            peers: RwLock::new(HashMap::new()),
            broadcast_tx,
            history: RwLock::new(history),
            db,
            start_time: std::time::Instant::now(),
            webhook,
            http_client: reqwest::Client::new(),
            rate_limits: RwLock::new(HashMap::new()),
            lockdown: RwLock::new(effective_lockdown),
            auto_lockdown: RwLock::new(false),
            kicked_keys: RwLock::new(HashSet::new()),
            connection_count: AtomicUsize::new(0),
            typing_timestamps: RwLock::new(HashMap::new()),
            upload_tokens: RwLock::new(HashMap::new()),
            voice_rooms: RwLock::new(HashMap::new()),
            user_statuses: RwLock::new(HashMap::new()),
            last_search_times: std::sync::Mutex::new(HashMap::new()),
            active_stream: RwLock::new(None),
            federation_connections: RwLock::new(HashMap::new()),
            federation_rate: std::sync::Mutex::new(HashMap::new()),
            vapid_key: None,
            server_config,
            game_world: RwLock::new(GameWorld::new()),
        }
    }

    /// Load or generate the VAPID keypair for WebPush.
    /// Persists the private key to `data/vapid_private.key` (raw ES256 bytes, base64url).
    pub fn init_vapid_key(&mut self) {
        use base64ct::{Base64UrlUnpadded, Encoding};

        let key_path = "data/vapid_private.key";
        if let Ok(encoded) = std::fs::read_to_string(key_path) {
            match Base64UrlUnpadded::decode_vec(encoded.trim()) {
                Ok(bytes) => match ES256KeyPair::from_bytes(&bytes) {
                    Ok(kp) => {
                        info!("VAPID key loaded from {key_path}");
                        self.vapid_key = Some(kp);
                        return;
                    }
                    Err(e) => tracing::error!("Failed to parse VAPID key: {e}"),
                },
                Err(e) => tracing::error!("Failed to decode VAPID key: {e}"),
            }
        }

        // Generate new keypair.
        let kp = ES256KeyPair::generate();
        let encoded = Base64UrlUnpadded::encode_string(&kp.to_bytes());
        if let Err(e) = std::fs::write(key_path, &encoded) {
            tracing::error!("Failed to write VAPID key to {key_path}: {e}");
            return;
        }
        info!("Generated new VAPID keypair → {key_path}");
        self.vapid_key = Some(kp);
    }

    /// Add a message to history, persist to DB, and broadcast it.
    pub async fn broadcast_and_store(&self, msg: RelayMessage) {
        // Persist to SQLite.
        if let Err(e) = self.db.store_message(&msg) {
            tracing::error!("Failed to persist message: {e}");
        }

        // Store in memory.
        {
            let mut history = self.history.write().await;
            history.push(msg.clone());
            // Trim if too long.
            if history.len() > MAX_HISTORY {
                let excess = history.len() - MAX_HISTORY;
                history.drain(..excess);
            }
        }
        // Broadcast to WebSocket clients.
        let _ = self.broadcast_tx.send(msg);
    }

    /// Fire webhook notification for a human message (non-bot).
    /// This is fire-and-forget — we don't block on the response.
    pub fn notify_webhook(&self, from_name: &str, content: &str) {
        let Some(ref webhook) = self.webhook else { return };

        let url = webhook.url.clone();
        let token = webhook.token.clone();
        let body = serde_json::json!({
            "text": format!("[Humanity Relay] {} says: {}", from_name, content),
            "mode": "now"
        });
        let client = self.http_client.clone();

        // Spawn fire-and-forget task.
        tokio::spawn(async move {
            let mut req = client.post(&url).json(&body);
            if let Some(t) = token {
                req = req.header("Authorization", format!("Bearer {t}"));
            }
            match req.send().await {
                Ok(resp) => {
                    if !resp.status().is_success() {
                        tracing::warn!("Webhook returned {}", resp.status());
                    }
                }
                Err(e) => {
                    tracing::warn!("Webhook failed: {e}");
                }
            }
        });
    }

    /// Send push notifications to all offline devices of a user.
    /// Fire-and-forget: spawns a tokio task, never blocks message routing.
    /// Automatically deletes stale subscriptions on 410 Gone.
    pub fn send_push_notification(
        self: &Arc<Self>,
        to_key: &str,
        title: &str,
        body: &str,
        tag: &str,
        url_path: &str,
    ) {
        use base64ct::{Base64UrlUnpadded, Encoding};
        use web_push_native::jwt_simple::prelude::ECDSAP256KeyPairLike;

        let Some(ref vapid_kp) = self.vapid_key else { return };

        let subs = self.db.get_push_subscriptions(to_key);
        if subs.is_empty() { return; }

        let payload = serde_json::json!({
            "title": title,
            "body": body,
            "tag": tag,
            "url": url_path,
        }).to_string();

        let client = self.http_client.clone();
        let vapid_bytes = vapid_kp.to_bytes();
        let state = self.clone();

        tokio::spawn(async move {
            let Ok(vapid_kp) = ES256KeyPair::from_bytes(&vapid_bytes) else { return };

            for sub in subs {
                // Decode subscription keys from base64url.
                let Ok(p256dh_bytes) = Base64UrlUnpadded::decode_vec(&sub.p256dh) else { continue };
                let Ok(auth_bytes) = Base64UrlUnpadded::decode_vec(&sub.auth) else { continue };
                let Ok(endpoint) = sub.endpoint.parse::<axum::http::Uri>() else { continue };
                let Ok(ua_public) = web_push_native::p256::PublicKey::from_sec1_bytes(&p256dh_bytes) else { continue };
                let ua_auth = web_push_native::Auth::clone_from_slice(&auth_bytes);

                let builder = web_push_native::WebPushBuilder::new(endpoint, ua_public, ua_auth)
                    .with_vapid(&vapid_kp, "mailto:admin@united-humanity.us");

                let http_req = match builder.build(payload.as_bytes()) {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!("Failed to build push message: {e}");
                        continue;
                    }
                };

                // Convert web_push_native http::Request to reqwest.
                let (parts, body_bytes) = http_req.into_parts();
                let url = parts.uri.to_string();
                let mut req = client.post(&url).body(body_bytes);
                for (k, v) in &parts.headers {
                    req = req.header(k.as_str(), v.to_str().unwrap_or_default());
                }

                match req.send().await {
                    Ok(resp) => {
                        let status = resp.status().as_u16();
                        if status == 410 || status == 404 {
                            // Subscription expired — remove it.
                            let _ = state.db.remove_push_subscription(&sub.endpoint);
                            tracing::info!("Removed stale push subscription ({})", status);
                        } else if status >= 400 {
                            tracing::warn!("Push failed for endpoint: HTTP {status}");
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Push request failed: {e}");
                    }
                }
            }
        });
    }
}

fn default_channel() -> String { "general".to_string() }

/// Channel info sent to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub read_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category_name: Option<String>,
    #[serde(default)]
    pub federated: bool,
}

/// Category info sent to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryInfo {
    pub id: i64,
    pub name: String,
    pub position: i32,
}

/// Link preview data sent with messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkPreview {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site_name: Option<String>,
}

/// Reply reference: embeds the original message context in a reply.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplyRef {
    pub from: String,
    pub from_name: String,
    pub content: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub public_key: String,
    pub label: Option<String>,
    pub registered_at: u64,
    pub is_current: bool,
    pub is_online: bool,
}

/// Messages sent over the relay WebSocket (JSON framing for MVP).
///
/// In production, these would be CBOR-encoded signed objects.
/// For the MVP, we use JSON to keep the web client simple.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RelayMessage {
    /// Client identifies itself with a public key and display name.
    #[serde(rename = "identify")]
    Identify {
        public_key: String,
        display_name: Option<String>,
        /// Optional link code for registering a new device under an existing name.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        link_code: Option<String>,
        /// Optional invite code for bypassing lockdown.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        invite_code: Option<String>,
        /// Required for bot_* keys: must match API_SECRET (L-1).
        #[serde(skip_serializing_if = "Option::is_none", default)]
        bot_secret: Option<String>,
        /// ECDH P-256 public key (base64 raw) for E2E encrypted DMs.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        ecdh_public: Option<String>,
    },

    /// A chat message, optionally Ed25519-signed.
    #[serde(rename = "chat")]
    Chat {
        from: String,
        from_name: Option<String>,
        content: String,
        timestamp: u64,
        /// Ed25519 signature hex (signs "{content}\n{timestamp}").
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
        /// Channel this message belongs to.
        #[serde(default = "default_channel")]
        channel: String,
        /// Reply reference: embeds the original message context.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        reply_to: Option<ReplyRef>,
        /// Number of replies to this message (populated server-side).
        #[serde(skip_serializing_if = "Option::is_none", default)]
        thread_count: Option<u32>,
        /// Database row ID (set after persistence; lets clients match message_deleted events).
        #[serde(skip_serializing_if = "Option::is_none", default)]
        message_id: Option<i64>,
    },

    /// Server announces a peer joined.
    #[serde(rename = "peer_joined")]
    PeerJoined {
        public_key: String,
        display_name: Option<String>,
        #[serde(default)]
        role: String,
        /// ECDH P-256 public key (base64 raw) for E2E encrypted DMs.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        ecdh_public: Option<String>,
    },

    /// Server announces a peer left.
    #[serde(rename = "peer_left")]
    PeerLeft {
        public_key: String,
    },

    /// Server sends the current peer list.
    #[serde(rename = "peer_list")]
    PeerList {
        peers: Vec<PeerInfo>,
        #[serde(skip_serializing_if = "Option::is_none")]
        server_version: Option<String>,
    },

    /// Server error or info.
    #[serde(rename = "system")]
    System {
        message: String,
    },

    /// Name is taken — client should pick a different name.
    #[serde(rename = "name_taken")]
    NameTaken {
        message: String,
    },

    /// Private system message — only delivered to a specific peer.
    /// The `to` field is checked by the send loop; it's stripped before sending.
    #[serde(rename = "private")]
    Private {
        to: String,
        message: String,
    },

    /// Server sends the list of available channels.
    #[serde(rename = "channel_list")]
    ChannelList {
        channels: Vec<ChannelInfo>,
        #[serde(skip_serializing_if = "Option::is_none")]
        categories: Option<Vec<CategoryInfo>>,
    },

    /// Typing indicator — broadcast to show who is composing a message.
    #[serde(rename = "typing")]
    Typing {
        from: String,
        from_name: Option<String>,
    },

    /// Delete a message — identified by sender key + timestamp.
    #[serde(rename = "delete")]
    Delete {
        from: String,
        timestamp: u64,
    },

    /// Emoji reaction on a message.
    #[serde(rename = "reaction")]
    Reaction {
        target_from: String,
        target_timestamp: u64,
        emoji: String,
        from: String,
        from_name: Option<String>,
        /// Channel this reaction belongs to (for persistence).
        #[serde(default = "default_channel")]
        channel: String,
    },

    /// Full user list (online + offline) for sidebar.
    #[serde(rename = "full_user_list")]
    FullUserList {
        users: Vec<UserInfo>,
    },

    /// Client sends a profile update (all fields; new fields are optional for backward compat).
    #[serde(rename = "profile_update")]
    ProfileUpdate {
        bio: String,
        socials: String,
        /// URL of the user's avatar image (max 512 chars, must start with https://).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        avatar_url: Option<String>,
        /// URL of the user's banner image (max 512 chars, must start with https://).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        banner_url: Option<String>,
        /// Pronouns string (max 64 chars, e.g. "she/her").
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pronouns: Option<String>,
        /// Location string (max 128 chars).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        location: Option<String>,
        /// Personal website URL (max 256 chars, must start with https://).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        website: Option<String>,
        /// JSON map of field → "private" / "public" visibility overrides.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        privacy: Option<String>,
    },

    /// Server sends profile data for a specific user.
    /// When `target` is Some(key) the message is unicast to that key only;
    /// when None it is broadcast to all connected peers (public fields only).
    #[serde(rename = "profile_data")]
    ProfileData {
        name: String,
        bio: String,
        socials: String,
        /// Only present when the requester has permission to see it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        avatar_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        banner_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pronouns: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        location: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        website: Option<String>,
        /// When Some(key): deliver only to that client. When None: broadcast to all.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
    },

    /// Client requests another user's profile.
    #[serde(rename = "profile_request")]
    ProfileRequest {
        name: String,
    },

    /// Server sends a batch of persisted reactions (on connect / channel switch).
    #[serde(rename = "reactions_sync")]
    ReactionsSync {
        reactions: Vec<ReactionData>,
    },

    /// Direct message between two users.
    #[serde(rename = "dm")]
    Dm {
        from: String,
        from_name: Option<String>,
        to: String,
        content: String,
        timestamp: u64,
        /// Whether this DM is end-to-end encrypted.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        encrypted: bool,
        /// Base64-encoded nonce/IV for encrypted DMs.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        nonce: Option<String>,
    },

    /// Client requests to open a DM conversation (load history).
    #[serde(rename = "dm_open")]
    DmOpen {
        partner: String,
    },

    /// Server sends DM conversation history.
    #[serde(rename = "dm_history")]
    DmHistory {
        /// Target recipient (stripped before sending to client).
        #[serde(skip_serializing_if = "Option::is_none", default)]
        target: Option<String>,
        partner: String,
        messages: Vec<DmData>,
    },

    /// Server sends list of DM conversations.
    #[serde(rename = "dm_list")]
    DmList {
        /// Target recipient (stripped before sending to client).
        #[serde(skip_serializing_if = "Option::is_none", default)]
        target: Option<String>,
        conversations: Vec<DmConversationData>,
    },

    /// Client marks DMs from a partner as read.
    #[serde(rename = "dm_read")]
    DmRead {
        partner: String,
    },

    /// Voice call signaling (ring/accept/reject/hangup) — forwarded peer-to-peer.
    #[serde(rename = "voice_call")]
    VoiceCall {
        from: String,
        from_name: Option<String>,
        to: String,
        action: String, // "ring" | "accept" | "reject" | "hangup"
    },

    /// WebRTC signaling (offer/answer/ICE) — forwarded peer-to-peer.
    #[serde(rename = "webrtc_signal")]
    WebrtcSignal {
        from: String,
        to: String,
        signal_type: String, // "offer" | "answer" | "ice"
        data: serde_json::Value,
    },

    /// Edit a message — identified by sender key + timestamp.
    #[serde(rename = "edit")]
    Edit {
        from: String,
        timestamp: u64,
        new_content: String,
        #[serde(default = "default_channel")]
        channel: String,
    },

    /// Client requests pinning a specific message by key+timestamp.
    #[serde(rename = "pin_request")]
    PinRequest {
        from_key: String,
        from_name: String,
        content: String,
        timestamp: u64,
        #[serde(default = "default_channel")]
        channel: String,
    },

    /// Server sends pinned messages sync on connect / channel switch.
    #[serde(rename = "pins_sync")]
    PinsSync {
        channel: String,
        pins: Vec<PinData>,
    },

    /// Server broadcasts when a message is pinned.
    #[serde(rename = "pin_added")]
    PinAdded {
        channel: String,
        pin: PinData,
    },

    /// Server broadcasts when a message is unpinned.
    #[serde(rename = "pin_removed")]
    PinRemoved {
        channel: String,
        index: usize,
    },

    /// Client sends a search query.
    #[serde(rename = "search")]
    Search {
        query: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        channel: Option<String>,
        /// Filter by sender display name (case-insensitive substring match).
        #[serde(skip_serializing_if = "Option::is_none", default)]
        from: Option<String>,
        /// Max results to return (default 50, max 100).
        #[serde(skip_serializing_if = "Option::is_none", default)]
        limit: Option<u32>,
    },

    /// Server responds with search results.
    #[serde(rename = "search_results")]
    SearchResults {
        /// Target client key (stripped before sending).
        #[serde(skip_serializing_if = "Option::is_none", default)]
        target: Option<String>,
        query: String,
        results: Vec<SearchResultData>,
        total: u32,
    },

    /// Delete a message by its database row ID (new style).
    #[serde(rename = "delete_by_id")]
    DeleteById {
        message_id: i64,
    },

    /// Server broadcasts that a message was deleted (by ID).
    #[serde(rename = "message_deleted")]
    MessageDeleted {
        message_id: i64,
        channel: String,
        from: String,
        timestamp: u64,
    },

    /// Client sets their status.
    #[serde(rename = "set_status")]
    SetStatus {
        status: String,
        #[serde(default)]
        text: String,
    },

    /// Voice room management.
    #[serde(rename = "voice_room")]
    VoiceRoom {
        action: String, // "create" | "join" | "leave" | "list"
        #[serde(skip_serializing_if = "Option::is_none", default)]
        room_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        room_name: Option<String>,
    },

    /// Server sends voice room state updates.
    #[serde(rename = "voice_room_update")]
    VoiceRoomUpdate {
        rooms: Vec<VoiceRoomData>,
    },

    /// Server sends persistent voice channel list (with current participants).
    #[serde(rename = "voice_channel_list")]
    VoiceChannelList {
        channels: Vec<VoiceChannelData>,
    },

    /// WebRTC signal for voice rooms (mesh topology).
    #[serde(rename = "voice_room_signal")]
    VoiceRoomSignal {
        from: String,
        to: String,
        room_id: String,
        signal_type: String,
        data: serde_json::Value,
    },

    /// Server sends link previews for URLs in a message.
    #[serde(rename = "link_previews")]
    LinkPreviews {
        from: String,
        timestamp: u64,
        channel: String,
        previews: Vec<LinkPreview>,
    },

    // ── Project Board messages ──

    /// Client requests the task list.
    #[serde(rename = "task_list")]
    TaskList {},

    /// Server responds with the full task list.
    #[serde(rename = "task_list_response")]
    TaskListResponse {
        #[serde(skip_serializing_if = "Option::is_none", default)]
        target: Option<String>,
        tasks: Vec<TaskData>,
    },

    /// Client creates a new task.
    #[serde(rename = "task_create")]
    TaskCreate {
        title: String,
        #[serde(default)]
        description: String,
        #[serde(default = "default_backlog")]
        status: String,
        #[serde(default = "default_medium")]
        priority: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        assignee: Option<String>,
        #[serde(default = "default_empty_labels")]
        labels: String,
        #[serde(default = "default_project")]
        project: String,
    },

    /// Server broadcasts a task was created.
    #[serde(rename = "task_created")]
    TaskCreated {
        task: TaskData,
    },

    /// Client updates a task.
    #[serde(rename = "task_update")]
    TaskUpdate {
        id: i64,
        title: String,
        #[serde(default)]
        description: String,
        #[serde(default = "default_medium")]
        priority: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        assignee: Option<String>,
        #[serde(default = "default_empty_labels")]
        labels: String,
        #[serde(default = "default_project")]
        project: String,
    },

    /// Server broadcasts a task was updated.
    #[serde(rename = "task_updated")]
    TaskUpdated {
        task: TaskData,
    },

    /// Client moves a task to a new status.
    #[serde(rename = "task_move")]
    TaskMove {
        id: i64,
        status: String,
    },

    /// Server broadcasts a task was moved.
    #[serde(rename = "task_moved")]
    TaskMoved {
        id: i64,
        status: String,
    },

    /// Client deletes a task.
    #[serde(rename = "task_delete")]
    TaskDelete {
        id: i64,
    },

    /// Server broadcasts a task was deleted.
    #[serde(rename = "task_deleted")]
    TaskDeleted {
        id: i64,
    },

    /// Client adds a comment to a task.
    #[serde(rename = "task_comment")]
    TaskComment {
        task_id: i64,
        content: String,
    },

    /// Server broadcasts a new comment was added.
    #[serde(rename = "task_comment_added")]
    TaskCommentAdded {
        task_id: i64,
        comment: TaskCommentData,
    },

    /// Client requests comments for a task.
    #[serde(rename = "task_comments_request")]
    TaskCommentsRequest {
        task_id: i64,
    },

    /// Server responds with comments for a task.
    #[serde(rename = "task_comments_response")]
    TaskCommentsResponse {
        #[serde(skip_serializing_if = "Option::is_none", default)]
        target: Option<String>,
        task_id: i64,
        comments: Vec<TaskCommentData>,
    },

    // ── Projects ──

    /// Client requests the project list.
    #[serde(rename = "project_list")]
    ProjectList {},

    /// Server responds with the project list.
    #[serde(rename = "project_list_response")]
    ProjectListResponse {
        #[serde(skip_serializing_if = "Option::is_none", default)]
        target: Option<String>,
        projects: Vec<ProjectData>,
    },

    /// Client creates a new project.
    #[serde(rename = "project_create")]
    ProjectCreate {
        name: String,
        #[serde(default)]
        description: String,
        #[serde(default = "default_public")]
        visibility: String,
        #[serde(default = "default_color")]
        color: String,
        #[serde(default = "default_icon")]
        icon: String,
    },

    /// Server broadcasts a project was created.
    #[serde(rename = "project_created")]
    ProjectCreated {
        project: ProjectData,
    },

    /// Client updates a project.
    #[serde(rename = "project_update")]
    ProjectUpdate {
        id: String,
        #[serde(default)]
        name: String,
        #[serde(default)]
        description: String,
        #[serde(default = "default_public")]
        visibility: String,
        #[serde(default = "default_color")]
        color: String,
        #[serde(default = "default_icon")]
        icon: String,
    },

    /// Server broadcasts a project was updated.
    #[serde(rename = "project_updated")]
    ProjectUpdated {
        project: ProjectData,
    },

    /// Client deletes a project.
    #[serde(rename = "project_delete")]
    ProjectDelete {
        id: String,
    },

    /// Server broadcasts a project was deleted.
    #[serde(rename = "project_deleted")]
    ProjectDeleted {
        id: String,
    },

    // ── Follow/Friend System ──

    /// Client requests to follow a user.
    #[serde(rename = "follow")]
    Follow {
        target_key: String,
    },

    /// Client requests to unfollow a user.
    #[serde(rename = "unfollow")]
    Unfollow {
        target_key: String,
    },

    /// Server sends the user's follow list on connect.
    #[serde(rename = "follow_list")]
    FollowList {
        #[serde(skip_serializing_if = "Option::is_none", default)]
        target: Option<String>,
        following: Vec<String>,
        followers: Vec<String>,
    },

    /// Server broadcasts follow/unfollow updates.
    #[serde(rename = "follow_update")]
    FollowUpdate {
        follower_key: String,
        followed_key: String,
        action: String, // "follow" | "unfollow"
    },

    // ── Friend Code System ──

    /// Client requests a friend code.
    #[serde(rename = "friend_code_request")]
    FriendCodeRequest {},

    /// Server responds with a generated friend code.
    #[serde(rename = "friend_code_response")]
    FriendCodeResponse {
        code: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        target: Option<String>,
    },

    /// Client redeems a friend code.
    #[serde(rename = "friend_code_redeem")]
    FriendCodeRedeem {
        code: String,
    },

    /// Server responds with friend code redemption result.
    #[serde(rename = "friend_code_result")]
    FriendCodeResult {
        success: bool,
        name: Option<String>,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        target: Option<String>,
    },

    // ── Server Membership messages ──

    /// Client requests paginated member list.
    #[serde(rename = "member_list")]
    MemberListRequest {
        #[serde(default)]
        limit: Option<usize>,
        #[serde(default)]
        offset: Option<usize>,
        #[serde(default)]
        search: Option<String>,
    },

    /// Server responds with member list (unicast to requester).
    #[serde(rename = "member_list_response")]
    MemberListResponse {
        #[serde(skip_serializing_if = "Option::is_none", default)]
        target: Option<String>,
        members: Vec<MemberData>,
        total: i64,
    },

    /// Server broadcasts when a new member joins.
    #[serde(rename = "member_joined")]
    MemberJoined {
        public_key: String,
        name: Option<String>,
        role: String,
    },

    // ── Group System ──

    // ── Marketplace messages ──

    /// Client requests to browse listings.
    #[serde(rename = "listing_browse")]
    ListingBrowse {},

    /// Client creates a listing.
    #[serde(rename = "listing_create")]
    ListingCreate {
        id: String,
        title: String,
        #[serde(default)]
        description: String,
        category: String,
        #[serde(default)]
        condition: String,
        #[serde(default)]
        price: String,
        #[serde(default)]
        payment_methods: String,
        #[serde(default)]
        location: String,
    },

    /// Client updates a listing.
    #[serde(rename = "listing_update")]
    ListingUpdate {
        id: String,
        title: String,
        #[serde(default)]
        description: String,
        category: String,
        #[serde(default)]
        condition: String,
        #[serde(default)]
        price: String,
        #[serde(default)]
        payment_methods: String,
        #[serde(default)]
        location: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        status: Option<String>,
    },

    /// Client deletes a listing.
    #[serde(rename = "listing_delete")]
    ListingDelete {
        id: String,
    },

    /// Server sends listing list to client.
    #[serde(rename = "listing_list")]
    ListingList {
        #[serde(skip_serializing_if = "Option::is_none", default)]
        target: Option<String>,
        listings: Vec<ListingData>,
    },

    /// Server broadcasts a new or updated listing.
    #[serde(rename = "listing_new")]
    ListingNew {
        listing: ListingData,
    },

    /// Server broadcasts that a listing was updated.
    #[serde(rename = "listing_updated")]
    ListingUpdated {
        listing: ListingData,
    },

    /// Server broadcasts that a listing was deleted.
    #[serde(rename = "listing_deleted")]
    ListingDeleted {
        id: String,
    },

    // ── Review messages ──

    /// Client creates a review for a listing.
    #[serde(rename = "review_create")]
    ReviewCreate {
        listing_id: String,
        rating: i32,
        #[serde(default)]
        comment: String,
    },

    /// Client deletes a review.
    #[serde(rename = "review_delete")]
    ReviewDelete {
        listing_id: String,
        review_id: i64,
    },

    /// Server broadcasts a new review.
    #[serde(rename = "review_created")]
    ReviewCreated {
        review: ReviewData,
    },

    /// Server broadcasts that a review was deleted.
    #[serde(rename = "review_deleted")]
    ReviewDeleted {
        listing_id: String,
        review_id: i64,
    },

    // ── Notification Preferences ──

    /// Client updates notification preferences.
    #[serde(rename = "update_notification_prefs")]
    UpdateNotificationPrefs {
        #[serde(default = "default_true")]
        dm: bool,
        #[serde(default = "default_true")]
        mentions: bool,
        #[serde(default = "default_true")]
        tasks: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dnd_start: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dnd_end: Option<String>,
    },

    /// Server sends notification preferences to the requesting client.
    #[serde(rename = "notification_prefs_data")]
    NotificationPrefsData {
        dm: bool,
        mentions: bool,
        tasks: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dnd_start: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dnd_end: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
    },

    /// Client requests their current notification preferences.
    #[serde(rename = "get_notification_prefs")]
    GetNotificationPrefs {},

    // ── Listing Messages (buyer-seller conversations) ──

    /// Client sends a message on a listing.
    #[serde(rename = "listing_message_send")]
    ListingMessageSend {
        listing_id: String,
        content: String,
    },

    /// Client requests message history for a listing.
    #[serde(rename = "listing_message_history")]
    ListingMessageHistory {
        listing_id: String,
    },

    /// Server sends listing messages to the requesting client.
    #[serde(rename = "listing_messages")]
    ListingMessages {
        listing_id: String,
        messages: Vec<ListingMessageData>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
    },

    /// Server broadcasts a new listing message.
    #[serde(rename = "listing_message_new")]
    ListingMessageNew {
        listing_id: String,
        message: ListingMessageData,
    },

    /// Client requests to create a group.
    #[serde(rename = "group_create")]
    GroupCreate {
        name: String,
    },

    /// Client requests to join a group by invite code.
    #[serde(rename = "group_join")]
    GroupJoin {
        invite_code: String,
    },

    /// Client requests to leave a group.
    #[serde(rename = "group_leave")]
    GroupLeave {
        group_id: String,
    },

    /// Client sends a message to a group.
    #[serde(rename = "group_msg")]
    GroupMsg {
        group_id: String,
        content: String,
    },

    /// Client requests group message history.
    #[serde(rename = "group_history_request")]
    GroupHistoryRequest {
        group_id: String,
    },

    /// Client requests group member list.
    #[serde(rename = "group_members_request")]
    GroupMembersRequest {
        group_id: String,
    },

    /// Client requests a thread (all replies to a specific message).
    #[serde(rename = "thread_request")]
    ThreadRequest {
        from: String,
        timestamp: u64,
    },

    /// Server responds with thread messages.
    #[serde(rename = "thread_response")]
    ThreadResponse {
        /// Target client key (stripped before sending).
        #[serde(skip_serializing_if = "Option::is_none", default)]
        target: Option<String>,
        /// The original message's from key.
        parent_from: String,
        /// The original message's timestamp.
        parent_timestamp: u64,
        /// All reply messages.
        messages: Vec<ThreadMessageData>,
    },

    /// Server sends group list to client.
    #[serde(rename = "group_list")]
    GroupList {
        #[serde(skip_serializing_if = "Option::is_none", default)]
        target: Option<String>,
        groups: Vec<GroupData>,
    },

    /// Server sends group message history.
    #[serde(rename = "group_history")]
    GroupHistory {
        #[serde(skip_serializing_if = "Option::is_none", default)]
        target: Option<String>,
        group_id: String,
        messages: Vec<GroupMessageData>,
    },

    /// Server broadcasts a group message.
    #[serde(rename = "group_message")]
    GroupMessage {
        group_id: String,
        from: String,
        from_name: Option<String>,
        content: String,
        timestamp: u64,
        /// Target member key — only deliver to this client (stripped before sending).
        #[serde(skip_serializing_if = "Option::is_none", default)]
        target: Option<String>,
    },

    /// Server sends group members list.
    #[serde(rename = "group_members")]
    GroupMembers {
        #[serde(skip_serializing_if = "Option::is_none", default)]
        target: Option<String>,
        group_id: String,
        members: Vec<(String, String)>, // (member_key, role)
    },

    /// Client requests their device list.
    #[serde(rename = "device_list_request")]
    DeviceListRequest {},

    /// Server responds with device list.
    #[serde(rename = "device_list")]
    DeviceList {
        devices: Vec<DeviceInfo>,
        #[serde(skip_serializing_if = "Option::is_none")]
        target: Option<String>,
    },

    /// Client labels a device key.
    #[serde(rename = "device_label")]
    DeviceLabel {
        public_key: String,
        label: String,
    },

    /// Client revokes a device key.
    #[serde(rename = "device_revoke")]
    DeviceRevoke {
        key_prefix: String,
    },

    // ── Federation Phase 2 ──

    /// Server-to-server federation handshake: hello.
    #[serde(rename = "federation_hello")]
    FederationHello {
        server_id: String,
        public_key: String,
        name: String,
        version: String,
        timestamp: u64,
        signature: String,
    },

    /// Server-to-server federation handshake: welcome response.
    #[serde(rename = "federation_welcome")]
    FederationWelcome {
        server_id: String,
        name: String,
        channels: Vec<String>,
    },

    /// Cross-server federated chat message.
    #[serde(rename = "federated_chat")]
    FederatedChat {
        server_id: String,
        server_name: String,
        from_name: String,
        content: String,
        timestamp: u64,
        channel: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },

    /// Federation status update (connection state of federated servers).
    #[serde(rename = "federation_status")]
    FederationStatus {
        servers: Vec<FederationServerStatus>,
    },

    /// Profile gossip between federated servers — signed profiles replicate everywhere.
    /// No home server: the signature is the authority. Latest timestamp wins.
    #[serde(rename = "profile_gossip")]
    ProfileGossip {
        public_key: String,
        name: String,
        bio: String,
        avatar_url: String,
        banner_url: String,
        socials: String,
        pronouns: String,
        location: String,
        website: String,
        timestamp: u64,
        signature: String,
    },

    // ── Streaming messages ──

    /// Admin starts a stream session.
    #[serde(rename = "stream_start")]
    StreamStart {
        title: String,
        #[serde(default)]
        category: String,
    },

    /// Stop the active stream.
    #[serde(rename = "stream_stop")]
    StreamStop {},

    /// WebRTC signaling: streamer sends offer to relay/viewer.
    #[serde(rename = "stream_offer")]
    StreamOffer {
        from: String,
        to: String,
        data: serde_json::Value,
    },

    /// WebRTC signaling: viewer sends answer back.
    #[serde(rename = "stream_answer")]
    StreamAnswer {
        from: String,
        to: String,
        data: serde_json::Value,
    },

    /// WebRTC signaling: ICE candidate exchange.
    #[serde(rename = "stream_ice")]
    StreamIce {
        from: String,
        to: String,
        data: serde_json::Value,
    },

    /// Viewer joins a stream.
    #[serde(rename = "stream_viewer_join")]
    StreamViewerJoin {
        #[serde(skip_serializing_if = "Option::is_none", default)]
        stream_id: Option<String>,
    },

    /// Viewer leaves a stream.
    #[serde(rename = "stream_viewer_leave")]
    StreamViewerLeave {
        #[serde(skip_serializing_if = "Option::is_none", default)]
        stream_id: Option<String>,
    },

    /// Stream chat message (unified: Humanity + external platforms).
    #[serde(rename = "stream_chat")]
    StreamChat {
        content: String,
        #[serde(default = "default_stream_source")]
        source: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        source_user: Option<String>,
        /// Populated server-side.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        from: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        from_name: Option<String>,
        #[serde(default)]
        timestamp: u64,
    },

    /// Server broadcasts current stream info to all clients.
    #[serde(rename = "stream_info")]
    StreamInfo {
        active: bool,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        streamer_name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        streamer_key: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        category: Option<String>,
        #[serde(default)]
        viewer_count: u32,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        started_at: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        external_urls: Option<Vec<StreamExternalUrl>>,
    },

    /// Client requests current stream info.
    #[serde(rename = "stream_info_request")]
    StreamInfoRequest {},

    /// Set external stream URLs (admin can add Twitch/YouTube/Rumble links).
    #[serde(rename = "stream_set_external")]
    StreamSetExternal {
        urls: Vec<StreamExternalUrl>,
    },

    // ── Peer-to-Peer Trading ──

    /// Client requests a trade with another user.
    #[serde(rename = "trade_request")]
    TradeRequest {
        target_key: String,
        #[serde(default)]
        message: String,
    },

    /// Client responds to a trade request (accept/reject).
    #[serde(rename = "trade_response")]
    TradeResponse {
        trade_id: String,
        accepted: bool,
    },

    /// Client updates items on their side of a trade.
    #[serde(rename = "trade_update_items")]
    TradeUpdateItems {
        trade_id: String,
        items: Vec<TradeItem>,
    },

    /// Client confirms a trade (both must confirm to complete).
    #[serde(rename = "trade_confirm")]
    TradeConfirm {
        trade_id: String,
    },

    /// Client cancels a trade.
    #[serde(rename = "trade_cancel")]
    TradeCancel {
        trade_id: String,
    },

    /// Server sends trade data to both parties.
    #[serde(rename = "trade_data")]
    TradeData {
        trade: TradeDataPayload,
    },

    /// Server notifies that a trade was completed.
    #[serde(rename = "trade_complete")]
    TradeComplete {
        trade_id: String,
    },

    /// Client requests list of their trades.
    #[serde(rename = "trade_list_request")]
    TradeListRequest {},

    /// Server responds with trade list.
    #[serde(rename = "trade_list")]
    TradeList {
        #[serde(skip_serializing_if = "Option::is_none", default)]
        target: Option<String>,
        trades: Vec<TradeDataPayload>,
    },
}

/// An item in a trade (flexible structure for any tradeable content).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeItem {
    pub item_type: String,
    pub name: String,
    #[serde(default)]
    pub quantity: u32,
    #[serde(default)]
    pub description: String,
    /// Optional ID for referencing specific items (listing IDs, etc.).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub reference_id: Option<String>,
}

/// Trade data sent to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeDataPayload {
    pub id: String,
    pub initiator_key: String,
    pub recipient_key: String,
    pub status: String,
    pub initiator_items: Vec<TradeItem>,
    pub recipient_items: Vec<TradeItem>,
    pub initiator_confirmed: bool,
    pub recipient_confirmed: bool,
    pub created_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

fn default_stream_source() -> String { "humanity".to_string() }

/// External stream URL (e.g. Twitch/YouTube/Rumble embed links).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamExternalUrl {
    pub platform: String,
    pub url: String,
}

/// Active stream state tracked in memory.
pub struct ActiveStream {
    pub streamer_key: String,
    pub streamer_name: String,
    pub title: String,
    pub category: String,
    pub started_at: u64,
    pub viewer_keys: HashSet<String>,
    pub external_urls: Vec<StreamExternalUrl>,
    pub db_id: Option<i64>,
}

/// Federation server connection status (sent to clients).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationServerStatus {
    pub server_id: String,
    pub name: String,
    pub connected: bool,
    pub trust_tier: i32,
    pub peer_count: Option<usize>,
}

/// A live connection to a federated server.
#[derive(Debug)]
pub struct FederatedConnection {
    pub tx: tokio::sync::mpsc::UnboundedSender<String>,
    pub server_id: String,
    pub server_name: String,
    pub trust_tier: i32,
    pub connected_at: Instant,
}

fn default_backlog() -> String { "backlog".to_string() }
fn default_medium() -> String { "medium".to_string() }
fn default_empty_labels() -> String { "[]".to_string() }
fn default_project() -> String { "default".to_string() }
fn default_public() -> String { "public".to_string() }
fn default_color() -> String { "#4488ff".to_string() }
fn default_icon() -> String { "\u{1F4CB}".to_string() }

/// Task data sent to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskData {
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
    pub comment_count: i64,
    #[serde(default = "default_project")]
    pub project: String,
}

/// Project data sent to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectData {
    pub id: String,
    pub name: String,
    pub description: String,
    pub owner_key: String,
    pub visibility: String,
    pub color: String,
    pub icon: String,
    pub created_at: String,
    pub task_count: i64,
}

/// Task comment data sent to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCommentData {
    pub id: i64,
    pub task_id: i64,
    pub author_key: String,
    pub author_name: String,
    pub content: String,
    pub created_at: i64,
}

/// Thread message data sent to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadMessageData {
    pub from: String,
    pub from_name: String,
    pub content: String,
    pub timestamp: u64,
    pub channel: String,
}

/// DM data sent to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DmData {
    pub from: String,
    pub from_name: String,
    pub to: String,
    pub content: String,
    pub timestamp: u64,
    /// Whether this DM is end-to-end encrypted.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub encrypted: bool,
    /// Base64-encoded nonce/IV for encrypted DMs.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub nonce: Option<String>,
}

/// DM conversation summary sent to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DmConversationData {
    pub partner_key: String,
    pub partner_name: String,
    pub last_message: String,
    pub last_timestamp: u64,
    pub unread_count: i64,
}

/// A single reaction record sent during sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionData {
    pub target_from: String,
    pub target_timestamp: u64,
    pub emoji: String,
    pub reactor_key: String,
    pub reactor_name: String,
}

/// A single pinned message record sent to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinData {
    pub from_key: String,
    pub from_name: String,
    pub content: String,
    pub original_timestamp: u64,
    pub pinned_by: String,
    pub pinned_at: u64,
}

/// A search result entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultData {
    pub message_id: i64,
    pub channel: String,
    pub from: String,
    pub from_name: String,
    pub content: String,
    pub timestamp: u64,
}

/// Voice room data sent to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceRoomData {
    pub room_id: String,
    pub name: String,
    pub participants: Vec<VoiceRoomParticipant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceRoomParticipant {
    pub public_key: String,
    pub display_name: String,
    pub muted: bool,
}

/// Persistent voice channel data sent to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceChannelData {
    pub id: i64,
    pub name: String,
    pub participants: Vec<VoiceRoomParticipant>,
}

/// In-memory voice room state (keyed by voice channel DB id as string).
pub struct VoiceRoom {
    pub name: String,
    pub participants: Vec<(String, String)>, // (public_key, display_name)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub public_key: String,
    pub display_name: Option<String>,
    #[serde(default)]
    pub role: String,
    /// Per-session upload token (only set for the recipient's own entry).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upload_token: Option<String>,
    /// User status: online, away, busy, dnd.
    #[serde(default = "default_online")]
    pub status: String,
    /// Custom status text.
    #[serde(default)]
    pub status_text: String,
    /// ECDH P-256 public key (base64 raw) for E2E encrypted DMs.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub ecdh_public: Option<String>,
}

fn default_true() -> bool { true }
fn default_online() -> String { "online".to_string() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupData {
    pub id: String,
    pub name: String,
    pub invite_code: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMessageData {
    pub from: String,
    pub from_name: String,
    pub content: String,
    pub timestamp: u64,
}

/// Listing data sent to clients.
/// Server member data for WebSocket responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberData {
    pub public_key: String,
    pub name: Option<String>,
    pub role: String,
    pub joined_at: String,
    pub last_seen: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListingData {
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

/// Review data sent to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewData {
    pub id: i64,
    pub listing_id: String,
    pub reviewer_key: String,
    pub reviewer_name: Option<String>,
    pub rating: i32,
    pub comment: String,
    pub created_at: String,
}

pub fn review_from_db(r: &crate::storage::ReviewRecord) -> ReviewData {
    ReviewData {
        id: r.id,
        listing_id: r.listing_id.clone(),
        reviewer_key: r.reviewer_key.clone(),
        reviewer_name: r.reviewer_name.clone(),
        rating: r.rating,
        comment: r.comment.clone(),
        created_at: r.created_at.clone(),
    }
}

/// Listing message data sent to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListingMessageData {
    pub id: i64,
    pub listing_id: String,
    pub sender_key: String,
    pub sender_name: Option<String>,
    pub content: String,
    pub timestamp: i64,
}

pub fn listing_message_from_db(m: &crate::storage::ListingMessageRecord) -> ListingMessageData {
    ListingMessageData {
        id: m.id,
        listing_id: m.listing_id.clone(),
        sender_key: m.sender_key.clone(),
        sender_name: m.sender_name.clone(),
        content: m.content.clone(),
        timestamp: m.timestamp,
    }
}

pub fn listing_from_db(l: &crate::storage::MarketplaceListing) -> ListingData {
    ListingData {
        id: l.id.clone(),
        seller_key: l.seller_key.clone(),
        seller_name: l.seller_name.clone(),
        title: l.title.clone(),
        description: l.description.clone(),
        category: l.category.clone(),
        condition: l.condition.clone(),
        price: l.price.clone(),
        payment_methods: l.payment_methods.clone(),
        location: l.location.clone(),
        images: l.images.clone(),
        status: l.status.clone(),
        created_at: l.created_at.clone(),
        updated_at: l.updated_at.clone(),
    }
}

/// Info about a registered user (online or offline) for the full user list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub name: String,
    pub public_key: String,
    pub role: String,
    pub online: bool,
    pub key_count: usize,
    #[serde(default = "default_online")]
    pub status: String,
    #[serde(default)]
    pub status_text: String,
    /// ECDH P-256 public key (base64 raw) for E2E encrypted DMs.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub ecdh_public: Option<String>,
}

/// RAII guard that decrements the connection counter when dropped.
/// Ensures accurate tracking regardless of how the connection handler exits.
struct ConnectionGuard {
    state: Arc<RelayState>,
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.state.connection_count.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Handle a single WebSocket connection.
pub async fn handle_connection(socket: WebSocket, state: Arc<RelayState>) {
    // Connection limit check: reject if at capacity.
    let prev = state.connection_count.fetch_add(1, Ordering::SeqCst);
    if prev >= MAX_CONNECTIONS {
        state.connection_count.fetch_sub(1, Ordering::SeqCst);
        tracing::warn!("Connection rejected: at capacity ({MAX_CONNECTIONS})");
        return;
    }

    // RAII guard ensures the counter is decremented on all exit paths.
    let _conn_guard = ConnectionGuard { state: state.clone() };

    let (mut ws_tx, mut ws_rx) = socket.split();
    let mut broadcast_rx = state.broadcast_tx.subscribe();
    let mut peer_key: Option<String> = None;

    // Wait for the identify message with a timeout.
    let identify_deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(IDENTIFY_TIMEOUT_SECS);

    loop {
        let msg = tokio::select! {
            msg = ws_rx.next() => msg,
            _ = tokio::time::sleep_until(identify_deadline) => {
                tracing::warn!("Client did not identify within {IDENTIFY_TIMEOUT_SECS}s, disconnecting");
                let _ = ws_tx.close().await;
                return;
            }
        };
        let Some(Ok(msg)) = msg else { return; };
        if let Message::Text(text) = msg {
            if let Ok(RelayMessage::Identify { public_key, display_name, link_code, invite_code, bot_secret, ecdh_public }) =
                serde_json::from_str::<RelayMessage>(&text)
            {
                // L-1: Bot keys require bot_secret matching API_SECRET.
                if public_key.starts_with("bot_") {
                    let expected = std::env::var("API_SECRET").unwrap_or_default();
                    let provided = bot_secret.as_deref().unwrap_or("");
                    // H-2: Use constant-time comparison to prevent timing attacks.
                    let ct_eq = provided.len() == expected.len() && provided.as_bytes().iter().zip(expected.as_bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0;
                    if expected.is_empty() || !ct_eq {
                        let err = RelayMessage::System {
                            message: "Bot authentication failed: invalid or missing bot_secret.".to_string(),
                        };
                        let _ = ws_tx.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                        let _ = ws_tx.close().await;
                        return;
                    }
                }
                let mut final_name = display_name.clone();

                // Handle link code redemption.
                if let Some(ref code) = link_code {
                    match state.db.redeem_link_code(code, &public_key) {
                        Ok(Some(linked_name)) => {
                            info!("Link code redeemed: {public_key} linked to name '{linked_name}'");
                            final_name = Some(linked_name);
                        }
                        Ok(None) => {
                            let err = RelayMessage::System {
                                message: "Invalid or expired link code.".to_string(),
                            };
                            let _ = ws_tx.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                            continue; // Let them retry
                        }
                        Err(e) => {
                            tracing::error!("Link code error: {e}");
                        }
                    }
                }

                // Validate name format: only letters, numbers, underscores, dashes.
                // WHY: Prevents homoglyph attacks (Cyrillic і vs Latin i, etc.)
                if let Some(ref name) = final_name {
                    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') || name.is_empty() || name.len() > 24 {
                        let err = RelayMessage::NameTaken {
                            message: "Names can only contain letters (A-Z), numbers, underscores, and dashes. Max 24 characters.".to_string(),
                        };
                        let _ = ws_tx.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                        continue;
                    }
                }

                // Check if user is banned.
                if !public_key.starts_with("bot_") {
                    if state.db.is_banned(&public_key).unwrap_or(false) {
                        let err = RelayMessage::NameTaken {
                            message: "You have been banned from this server.".to_string(),
                        };
                        let _ = ws_tx.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                        let _ = ws_tx.close().await;
                        return;
                    }
                }

                // Check name registration (skip for bot keys).
                if !public_key.starts_with("bot_") {
                    if let Some(ref name) = final_name {
                        match state.db.check_name(name, &public_key) {
                            Ok(None) => {
                                // Name is free — check lockdown before registering.
                                let locked = *state.lockdown.read().await;
                                if locked {
                                    // Check for invite code bypass.
                                    let mut invite_ok = false;
                                    if let Some(ref code) = invite_code {
                                        match state.db.redeem_invite_code(code, &public_key) {
                                            Ok(true) => {
                                                invite_ok = true;
                                                info!("Invite code redeemed by {public_key} during lockdown");
                                            }
                                            Ok(false) => {}
                                            Err(e) => {
                                                tracing::error!("Invite code error: {e}");
                                            }
                                        }
                                    }
                                    if !invite_ok {
                                        let err = RelayMessage::NameTaken {
                                            message: "🔒 Registration is currently locked. Only existing users can connect. Use an invite code to bypass.".to_string(),
                                        };
                                        let _ = ws_tx.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                                        continue;
                                    }
                                }
                                if let Err(e) = state.db.register_name(name, &public_key) {
                                    tracing::error!("Failed to register name: {e}");
                                }
                                info!("Name '{name}' registered to {public_key}");
                            }
                            Ok(Some(true)) => {
                                // Key is authorized for this name — all good.
                                info!("Name '{name}' authorized for {public_key}");
                            }
                            Ok(Some(false)) => {
                                // Name taken by someone else!
                                let err = RelayMessage::NameTaken {
                                    message: format!("The name '{}' is already registered to another identity. Please choose a different name or use a link code from the owner.", name),
                                };
                                let _ = ws_tx.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                                continue; // Let them retry with a different name
                            }
                            Err(e) => {
                                tracing::error!("Name check error: {e}");
                            }
                        }
                    }
                }

                // M-4: Generate per-session upload token.
                let upload_token = {
                    let random_bytes: [u8; 16] = rand::rng().random();
                    hex::encode(random_bytes)
                };

                // Store ECDH public key in DB if provided.
                if let Some(ref ecdh_key) = ecdh_public {
                    if let Err(e) = state.db.store_ecdh_public(&public_key, ecdh_key) {
                        tracing::error!("Failed to store ECDH public key: {e}");
                    }
                }

                let peer = Peer {
                    public_key_hex: public_key.clone(),
                    display_name: final_name.clone(),
                    upload_token: Some(upload_token.clone()),
                    ecdh_public: ecdh_public.clone(),
                };

                // Register peer and upload token mapping.
                state.peers.write().await.insert(public_key.clone(), peer);
                state.upload_tokens.write().await.insert(upload_token.clone(), public_key.clone());
                peer_key = Some(public_key.clone());

                // Load persisted user status.
                if let Some(ref name) = final_name {
                    if let Ok(Some((status, status_text))) = state.db.load_user_status(name) {
                        state.user_statuses.write().await.insert(name.to_lowercase(), (status, status_text));
                    }
                }

                info!("Peer connected: {public_key} ({:?})", final_name);

                // Auto-join server membership: if user has a name and isn't a bot,
                // auto-register them as a member (open server model).
                if !public_key.starts_with("bot_") && !public_key.starts_with("viewer_") {
                    let member_name = final_name.as_deref().unwrap_or("Anonymous");
                    if state.db.is_member(&public_key) {
                        // Already a member — update last_seen and name.
                        let _ = state.db.update_last_seen(&public_key);
                        let _ = state.db.update_member_name(&public_key, member_name);
                    } else {
                        // New member — auto-join and broadcast.
                        if let Ok(true) = state.db.join_server(&public_key, member_name) {
                            info!("Auto-joined member: {public_key} as '{member_name}'");
                            let _ = state.broadcast_tx.send(RelayMessage::MemberJoined {
                                public_key: public_key.clone(),
                                name: final_name.clone(),
                                role: "member".to_string(),
                            });
                        }
                    }
                }

                // Send current peer list to the new peer (with their upload_token).
                let peers_raw: Vec<Peer> = state
                    .peers
                    .read()
                    .await
                    .values()
                    .cloned()
                    .collect::<Vec<_>>();
                let statuses_snap = state.user_statuses.read().await;
                let peers: Vec<PeerInfo> = peers_raw.into_iter()
                    .map(|p| {
                        let role = state.db.get_role(&p.public_key_hex).unwrap_or_default();
                        let token = if p.public_key_hex == public_key {
                            p.upload_token.clone()
                        } else {
                            None
                        };
                        let name_lower = p.display_name.as_ref().map(|n| n.to_lowercase()).unwrap_or_default();
                        let (user_status, user_status_text) = statuses_snap.get(&name_lower).cloned().unwrap_or(("online".to_string(), String::new()));
                        // Get ECDH key: from in-memory peer (if online) or from DB.
                        let ecdh_pub = p.ecdh_public.clone().or_else(|| state.db.get_ecdh_public(&p.public_key_hex).ok().flatten());
                        PeerInfo {
                            public_key: p.public_key_hex.clone(),
                            display_name: p.display_name.clone(),
                            role,
                            upload_token: token,
                            status: user_status,
                            status_text: user_status_text,
                            ecdh_public: ecdh_pub,
                        }
                    })
                    .collect();
                drop(statuses_snap);

                let list_msg = serde_json::to_string(&RelayMessage::PeerList {
                    peers,
                    server_version: Some(env!("BUILD_VERSION").to_string()),
                }).unwrap();
                let _ = ws_tx.send(Message::Text(list_msg.into())).await;

                // Send full user list (online + offline) to the new peer.
                if let Ok(all_users) = state.db.list_all_users_with_keys() {
                    let online_names: std::collections::HashSet<String> = state
                        .peers
                        .read()
                        .await
                        .values()
                        .filter_map(|p| p.display_name.clone())
                        .map(|n| n.to_lowercase())
                        .collect();
                    let statuses_snap2 = state.user_statuses.read().await;
                    let users: Vec<UserInfo> = all_users
                        .into_iter()
                        .map(|(name, first_key, role, key_count)| {
                            // Bot accounts (bot_ prefix keys) are always shown as online.
                            let online = first_key.starts_with("bot_") || online_names.contains(&name.to_lowercase());
                            let (us, ust) = statuses_snap2.get(&name.to_lowercase()).cloned().unwrap_or(("online".to_string(), String::new()));
                            let ecdh_pub = state.db.get_ecdh_public(&first_key).ok().flatten();
                            UserInfo { name, public_key: first_key, role, online, key_count, status: us, status_text: ust, ecdh_public: ecdh_pub }
                        })
                        .collect();
                    drop(statuses_snap2);
                    let ful_msg = serde_json::to_string(&RelayMessage::FullUserList { users }).unwrap();
                    let _ = ws_tx.send(Message::Text(ful_msg.into())).await;
                }

                // Send the user's own full profile on connect so the client can pre-fill the edit modal.
                if let Some(ref name) = final_name {
                    if let Ok(Some((bio, socials, avatar_url, banner_url, pronouns, location, website, _privacy))) =
                        state.db.get_profile_extended(name)
                    {
                        let opt = |s: String| if s.is_empty() { None } else { Some(s) };
                        let profile_msg = serde_json::to_string(&RelayMessage::ProfileData {
                            name: name.clone(),
                            bio,
                            socials,
                            avatar_url: opt(avatar_url),
                            banner_url: opt(banner_url),
                            pronouns:   opt(pronouns),
                            location:   opt(location),
                            website:    opt(website),
                            target: None, // Direct send to this socket — no routing needed
                        }).unwrap();
                        let _ = ws_tx.send(Message::Text(profile_msg.into())).await;
                    }
                }

                // Send channel list with category info.
                {
                    let channel_infos = build_channel_list(&state.db);
                    let categories: Vec<CategoryInfo> = state.db.list_categories().unwrap_or_default().into_iter()
                        .map(|(id, name, pos)| CategoryInfo { id, name, position: pos }).collect();
                    let ch_msg = serde_json::to_string(&RelayMessage::ChannelList { channels: channel_infos, categories: Some(categories) }).unwrap();
                    let _ = ws_tx.send(Message::Text(ch_msg.into())).await;
                }

                // Send persisted reactions for the default channel ("general").
                if let Ok(records) = state.db.load_channel_reactions("general", 500) {
                    let reactions: Vec<ReactionData> = records.into_iter().map(|r| ReactionData {
                        target_from: r.target_from,
                        target_timestamp: r.target_timestamp,
                        emoji: r.emoji,
                        reactor_key: r.reactor_key,
                        reactor_name: r.reactor_name,
                    }).collect();
                    if !reactions.is_empty() {
                        let sync_msg = serde_json::to_string(&RelayMessage::ReactionsSync { reactions }).unwrap();
                        let _ = ws_tx.send(Message::Text(sync_msg.into())).await;
                    }
                }

                // Send pinned messages for the default channel ("general").
                if let Ok(pins) = state.db.get_pinned_messages("general") {
                    let pin_data: Vec<PinData> = pins.into_iter().map(|p| PinData {
                        from_key: p.from_key,
                        from_name: p.from_name,
                        content: p.content,
                        original_timestamp: p.original_timestamp,
                        pinned_by: p.pinned_by,
                        pinned_at: p.pinned_at,
                    }).collect();
                    let pins_msg = serde_json::to_string(&RelayMessage::PinsSync {
                        channel: "general".to_string(),
                        pins: pin_data,
                    }).unwrap();
                    let _ = ws_tx.send(Message::Text(pins_msg.into())).await;
                }

                // Send DM conversation list to the new peer.
                if let Ok(convos) = state.db.get_dm_conversations(&public_key) {
                    let conversations: Vec<DmConversationData> = convos.into_iter().map(|c| DmConversationData {
                        partner_key: c.partner_key,
                        partner_name: c.partner_name,
                        last_message: c.last_message,
                        last_timestamp: c.last_timestamp,
                        unread_count: c.unread_count,
                    }).collect();
                    if !conversations.is_empty() {
                        let dm_list_msg = serde_json::to_string(&RelayMessage::DmList {
                            target: None, // Direct send, not via broadcast
                            conversations,
                        }).unwrap();
                        let _ = ws_tx.send(Message::Text(dm_list_msg.into())).await;
                    }
                }

                // Send follow list to the new peer.
                {
                    let following = state.db.get_following(&public_key).unwrap_or_default();
                    let followers = state.db.get_followers(&public_key).unwrap_or_default();
                    if !following.is_empty() || !followers.is_empty() {
                        let follow_msg = serde_json::to_string(&RelayMessage::FollowList {
                            target: None,
                            following,
                            followers,
                        }).unwrap();
                        let _ = ws_tx.send(Message::Text(follow_msg.into())).await;
                    }
                }

                // Send group list to the new peer.
                {
                    if let Ok(user_groups) = state.db.get_user_groups(&public_key) {
                        if !user_groups.is_empty() {
                            let groups: Vec<GroupData> = user_groups.into_iter().map(|(id, name, invite_code, role)| {
                                GroupData { id, name, invite_code, role }
                            }).collect();
                            let group_msg = serde_json::to_string(&RelayMessage::GroupList {
                                target: None,
                                groups,
                            }).unwrap();
                            let _ = ws_tx.send(Message::Text(group_msg.into())).await;
                        }
                    }
                }

                // Send persistent voice channel list with current participants.
                {
                    let vc_msg = build_voice_channel_list_msg(&state).await;
                    let _ = ws_tx.send(Message::Text(serde_json::to_string(&vc_msg).unwrap().into())).await;
                }

                // Announce to everyone.
                let peer_role = state.db.get_role(&public_key).unwrap_or_default();
                let _ = state.broadcast_tx.send(RelayMessage::PeerJoined {
                    public_key,
                    display_name: final_name,
                    role: peer_role.clone(),
                    ecdh_public: ecdh_public.clone(),
                });

                // Broadcast updated full user list to all clients.
                broadcast_full_user_list(&state).await;

                // Auto-unlock: if an admin/mod connects and lockdown was auto-set, lift it.
                if peer_role == "admin" || peer_role == "mod" {
                    let is_auto = *state.auto_lockdown.read().await;
                    if is_auto {
                        let locked = *state.lockdown.read().await;
                        if locked {
                            *state.lockdown.write().await = false;
                            *state.auto_lockdown.write().await = false;
                            // L-4: Persist lockdown state.
                            let _ = state.db.set_state("lockdown", "false");
                            let sys = RelayMessage::System {
                                message: "🔓 Auto-unlock: moderator online.".to_string(),
                            };
                            let _ = state.broadcast_tx.send(sys);
                        }
                    }
                }

                break;
            }
        }
    }

    let Some(my_key) = peer_key.clone() else {
        return; // Connection closed before identifying.
    };

    // Spawn a task to forward broadcast messages to this client.
    let my_key_for_broadcast = my_key.clone();
    let state_for_broadcast = state.clone();
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = broadcast_rx.recv().await {
            // Check if this user has been kicked — if so, close the connection.
            if state_for_broadcast.kicked_keys.read().await.contains(&my_key_for_broadcast) {
                let kick_notice = RelayMessage::System {
                    message: "You have been kicked.".to_string(),
                };
                let json = serde_json::to_string(&kick_notice).unwrap();
                let _ = ws_tx.send(Message::Text(json.into())).await;
                let _ = ws_tx.close().await;
                break;
            }

            // Don't echo typing indicators back to the sender.
            // Chat messages ARE echoed to support multi-device (web + native same key).
            // Each client deduplicates by timestamp if needed.
            let should_skip = match &msg {
                RelayMessage::Typing { from, .. } => from == &my_key_for_broadcast,
                _ => false,
            };
            if should_skip {
                continue;
            }

            // DM messages: only deliver to the targeted recipient (not sender — sender gets a confirmation copy via separate send).
            if let RelayMessage::Dm { ref to, .. } = msg {
                if to != &my_key_for_broadcast {
                    continue; // Not for us
                }
                // Fall through to send it
            }

            // VoiceCall: only deliver to the target peer.
            if let RelayMessage::VoiceCall { ref to, .. } = msg {
                if to != &my_key_for_broadcast {
                    continue;
                }
            }

            // WebrtcSignal: only deliver to the target peer.
            if let RelayMessage::WebrtcSignal { ref to, .. } = msg {
                if to != &my_key_for_broadcast {
                    continue;
                }
            }

            // ProfileData: when target is set deliver only to that client; when None broadcast to all.
            if let RelayMessage::ProfileData { ref target, .. } = msg {
                match target {
                    Some(t) if t != &my_key_for_broadcast => continue,
                    None => {} // No target = broadcast to everyone, fall through
                    _ => {}   // Target matches, fall through
                }
            }

            // DmHistory: only deliver to the target client.
            if let RelayMessage::DmHistory { ref target, .. } = msg {
                match target {
                    Some(t) if t != &my_key_for_broadcast => continue,
                    None => continue, // No target = skip
                    _ => {} // Target matches, fall through
                }
            }

            // DmList: only deliver to the target client.
            if let RelayMessage::DmList { ref target, .. } = msg {
                match target {
                    Some(t) if t != &my_key_for_broadcast => continue,
                    None => continue,
                    _ => {}
                }
            }

            // SearchResults: only deliver to the target client.
            if let RelayMessage::SearchResults { ref target, .. } = msg {
                match target {
                    Some(t) if t != &my_key_for_broadcast => continue,
                    None => continue,
                    _ => {}
                }
            }

            // TaskListResponse: only deliver to the target client.
            if let RelayMessage::TaskListResponse { ref target, .. } = msg {
                match target {
                    Some(t) if t != &my_key_for_broadcast => continue,
                    None => continue,
                    _ => {}
                }
            }

            // TaskCommentsResponse: only deliver to the target client.
            if let RelayMessage::TaskCommentsResponse { ref target, .. } = msg {
                match target {
                    Some(t) if t != &my_key_for_broadcast => continue,
                    None => continue,
                    _ => {}
                }
            }

            // ProjectListResponse: only deliver to the target client.
            if let RelayMessage::ProjectListResponse { ref target, .. } = msg {
                match target {
                    Some(t) if t != &my_key_for_broadcast => continue,
                    None => continue,
                    _ => {}
                }
            }

            // H-5: GroupMessage — only deliver to targeted group member.
            if let RelayMessage::GroupMessage { ref target, .. } = msg {
                match target {
                    Some(t) if t != &my_key_for_broadcast => continue,
                    None => continue,
                    _ => {}
                }
            }

            // VoiceRoomSignal: only deliver to the target peer.
            if let RelayMessage::VoiceRoomSignal { ref to, .. } = msg {
                if to != &my_key_for_broadcast {
                    continue;
                }
            }

            // Stream signaling: only deliver to the target peer.
            if let RelayMessage::StreamOffer { ref to, .. } = msg {
                if to != &my_key_for_broadcast { continue; }
            }
            if let RelayMessage::StreamAnswer { ref to, .. } = msg {
                if to != &my_key_for_broadcast { continue; }
            }
            if let RelayMessage::StreamIce { ref to, .. } = msg {
                if to != &my_key_for_broadcast { continue; }
            }

            // FollowList: only deliver to the target client.
            if let RelayMessage::FollowList { ref target, .. } = msg {
                match target {
                    Some(t) if t != &my_key_for_broadcast => continue,
                    None => continue,
                    _ => {}
                }
            }

            // GroupList: only deliver to the target client.
            if let RelayMessage::GroupList { ref target, .. } = msg {
                match target {
                    Some(t) if t != &my_key_for_broadcast => continue,
                    None => continue,
                    _ => {}
                }
            }

            // GroupMembers: only deliver to the target client.
            if let RelayMessage::GroupMembers { ref target, .. } = msg {
                match target {
                    Some(t) if t != &my_key_for_broadcast => continue,
                    None => continue,
                    _ => {}
                }
            }

            // FriendCodeResponse: only deliver to the target client.
            if let RelayMessage::FriendCodeResponse { ref target, .. } = msg {
                match target {
                    Some(t) if t != &my_key_for_broadcast => continue,
                    None => continue,
                    _ => {}
                }
            }

            // FriendCodeResult: only deliver to the target client.
            if let RelayMessage::FriendCodeResult { ref target, .. } = msg {
                match target {
                    Some(t) if t != &my_key_for_broadcast => continue,
                    None => continue,
                    _ => {}
                }
            }

            // ListingList: only deliver to the target client.
            if let RelayMessage::ListingList { ref target, .. } = msg {
                match target {
                    Some(t) if t != &my_key_for_broadcast => continue,
                    None => continue,
                    _ => {}
                }
            }

            // NotificationPrefsData: only deliver to the target client.
            if let RelayMessage::NotificationPrefsData { ref target, .. } = msg {
                match target {
                    Some(t) if t != &my_key_for_broadcast => continue,
                    None => continue,
                    _ => {}
                }
            }

            // ListingMessages: only deliver to the target client.
            if let RelayMessage::ListingMessages { ref target, .. } = msg {
                match target {
                    Some(t) if t != &my_key_for_broadcast => continue,
                    None => continue,
                    _ => {}
                }
            }

            // MemberListResponse: only deliver to the target client.
            if let RelayMessage::MemberListResponse { ref target, .. } = msg {
                match target {
                    Some(t) if t != &my_key_for_broadcast => continue,
                    None => continue,
                    _ => {}
                }
            }

            // GroupHistory: only deliver to the target client.
            if let RelayMessage::GroupHistory { ref target, .. } = msg {
                match target {
                    Some(t) if t != &my_key_for_broadcast => continue,
                    None => continue,
                    _ => {}
                }
            }

            // ThreadResponse: only deliver to the target client.
            if let RelayMessage::ThreadResponse { ref target, .. } = msg {
                match target {
                    Some(t) if t != &my_key_for_broadcast => continue,
                    None => continue,
                    _ => {}
                }
            }

            // DeviceList: only deliver to the target client.
            if let RelayMessage::DeviceList { ref target, .. } = msg {
                match target {
                    Some(t) if t != &my_key_for_broadcast => continue,
                    None => continue,
                    _ => {}
                }
            }

            // Private messages: only deliver to the targeted peer.
            if let RelayMessage::Private { ref to, ref message } = msg {
                if to != &my_key_for_broadcast {
                    continue; // Not for us
                }
                // Convert to a regular system message before sending.
                let sys = RelayMessage::System { message: message.clone() };
                let json = serde_json::to_string(&sys).unwrap();
                if ws_tx.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
                continue;
            }

            let json = serde_json::to_string(&msg).unwrap();
            if ws_tx.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    });

    // Read incoming messages from the client.
    let state_clone = state.clone();
    let my_key_for_recv = my_key.clone();
    let mut last_profile_update: Option<Instant> = None;
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            // Check if this user has been kicked — stop processing messages.
            if state_clone.kicked_keys.read().await.contains(&my_key_for_recv) {
                break;
            }
            match msg {
                Message::Text(text) => {
                    // Handle sync messages (not part of RelayMessage enum).
                    if let Ok(raw) = serde_json::from_str::<serde_json::Value>(&text) {
                        match raw.get("type").and_then(|t| t.as_str()) {
                            Some("sync_save") => {
                                handle_sync_save(&state_clone, &my_key_for_recv, &raw).await;
                                continue;
                            }
                            Some("sync_load") => {
                                handle_sync_load(&state_clone, &my_key_for_recv).await;
                                continue;
                            }
                            Some("key_rotation") => {
                                handle_key_rotation(&state_clone, &my_key_for_recv, &raw).await;
                                continue;
                            }
                            Some("skill_update") => {
                                handle_skill_update(&state_clone, &my_key_for_recv, &raw).await;
                                continue;
                            }
                            Some("skill_verify_request") => {
                                handle_skill_verify_request(&state_clone, &my_key_for_recv, &raw).await;
                                continue;
                            }
                            Some("skill_verify_response") => {
                                handle_skill_verify_response(&state_clone, &my_key_for_recv, &raw).await;
                                continue;
                            }
                            Some("skill_endorsements_request") => {
                                handle_skill_endorsements_request(&state_clone, &my_key_for_recv, &raw).await;
                                continue;
                            }
                            Some("task_update") => {
                                handle_raw_task_update(&state_clone, &my_key_for_recv, &raw).await;
                                continue;
                            }
                            Some("task_comment") => {
                                handle_raw_task_comment(&state_clone, &my_key_for_recv, &raw).await;
                                continue;
                            }
                            Some("task_create") => {
                                handle_raw_task_create(&state_clone, &my_key_for_recv, &raw).await;
                                continue;
                            }
                            // ── Trade messages ──
                            Some("trade_request") => {
                                handle_trade_request(&state_clone, &my_key_for_recv, &raw).await;
                                continue;
                            }
                            Some("trade_response") => {
                                handle_trade_response(&state_clone, &my_key_for_recv, &raw).await;
                                continue;
                            }
                            Some("trade_update_items") => {
                                handle_trade_update_items(&state_clone, &my_key_for_recv, &raw).await;
                                continue;
                            }
                            Some("trade_confirm") => {
                                handle_trade_confirm(&state_clone, &my_key_for_recv, &raw).await;
                                continue;
                            }
                            Some("trade_cancel") => {
                                handle_trade_cancel(&state_clone, &my_key_for_recv, &raw).await;
                                continue;
                            }
                            Some("trade_list_request") => {
                                handle_trade_list_request(&state_clone, &my_key_for_recv).await;
                                continue;
                            }
                            // ── Game state messages ──
                            Some("game_join") => {
                                handle_game_join(&state_clone, &my_key_for_recv, &raw).await;
                                continue;
                            }
                            Some("game_position_update") => {
                                handle_game_position_update(&state_clone, &my_key_for_recv, &raw).await;
                                continue;
                            }
                            _ => {} // Fall through to normal RelayMessage handling
                        }
                    }
                    if let Err(deser_err) = serde_json::from_str::<RelayMessage>(&text).as_ref() {
                        tracing::warn!("RelayMessage deserialization failed: {} | raw: {}", deser_err, &text[..text.len().min(200)]);
                    }
                    if let Ok(relay_msg) = serde_json::from_str::<RelayMessage>(&text) {
                        match relay_msg {
                            RelayMessage::Chat { content, timestamp, signature, channel, reply_to, .. } => {
                                let peer = state_clone
                                    .peers
                                    .read()
                                    .await
                                    .get(&my_key_for_recv)
                                    .cloned();

                                let display = peer.as_ref()
                                    .and_then(|p| p.display_name.clone())
                                    .unwrap_or_else(|| "Anonymous".to_string());

                                // Check if user is muted.
                                let user_role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                if user_role == "muted" {
                                    let private = RelayMessage::Private {
                                        to: my_key_for_recv.clone(),
                                        message: "You are muted and cannot send messages.".to_string(),
                                    };
                                    let _ = state_clone.broadcast_tx.send(private);
                                    continue;
                                }

                                // Rate limiting: skip for bots and admins.
                                if !my_key_for_recv.starts_with("bot_") && user_role != "admin" {
                                    let now = Instant::now();
                                    let mut rate_limits = state_clone.rate_limits.write().await;
                                    let rl = rate_limits.entry(my_key_for_recv.clone()).or_insert_with(|| {
                                        // Use DB registered_at so relay restarts don't retroactively
                                        // slow-mode established accounts (in-memory first_seen resets on restart).
                                        let unix_now = std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap_or_default().as_secs();
                                        let reg_at = {
                                            let conn = state_clone.db.conn.lock().unwrap();
                                            conn.query_row(
                                                "SELECT MIN(registered_at) FROM registered_names WHERE public_key = ?1",
                                                rusqlite::params![&my_key_for_recv],
                                                |row| row.get::<_, Option<i64>>(0),
                                            ).ok().flatten().unwrap_or(unix_now as i64) as u64
                                        };
                                        let account_age = unix_now.saturating_sub(reg_at);
                                        let first_seen = if account_age >= NEW_ACCOUNT_WINDOW_SECS {
                                            now - std::time::Duration::from_secs(NEW_ACCOUNT_WINDOW_SECS + 1)
                                        } else {
                                            now - std::time::Duration::from_secs(account_age)
                                        };
                                        RateLimitState {
                                            first_seen,
                                            last_message_time: now - std::time::Duration::from_secs(60), // allow first message
                                            fib_index: 0,
                                        }
                                    });

                                    let elapsed = now.duration_since(rl.last_message_time).as_secs();

                                    // Determine required delay: Fibonacci backoff.
                                    let fib_delay = FIB_DELAYS[rl.fib_index];

                                    // New-account slow mode: if first seen < 10 min ago, min 5s delay.
                                    // Skip for verified, mod, and admin users.
                                    let is_trusted = user_role == "verified" || user_role == "donor" || user_role == "mod" || user_role == "admin";
                                    let account_age = now.duration_since(rl.first_seen).as_secs();
                                    let new_account_delay = if !is_trusted && account_age < NEW_ACCOUNT_WINDOW_SECS {
                                        NEW_ACCOUNT_DELAY_SECS
                                    } else {
                                        0
                                    };

                                    // Use whichever delay is longer.
                                    let required_delay = fib_delay.max(new_account_delay);

                                    if elapsed < required_delay {
                                        let wait = required_delay - elapsed;
                                        let private = RelayMessage::Private {
                                            to: my_key_for_recv.clone(),
                                            message: format!("⏳ Slow down! Please wait {} more second{}.", wait, if wait == 1 { "" } else { "s" }),
                                        };
                                        let _ = state_clone.broadcast_tx.send(private);
                                        continue;
                                    }

                                    // User waited long enough — check if we should reset or advance.
                                    if elapsed > required_delay {
                                        // User waited longer than needed — reset to position 0.
                                        rl.fib_index = 0;
                                    } else {
                                        // User sent exactly at the boundary — advance Fibonacci.
                                        rl.fib_index = (rl.fib_index + 1).min(FIB_DELAYS.len() - 1);
                                    }

                                    rl.last_message_time = now;
                                }

                                // Enforce max message length (admins: 10000, others: 2000).
                                // Quotes (lines starting with "> ") are exempt.
                                let char_limit: usize = if user_role == "admin" { 10_000 } else { 2_000 };
                                let user_text_len: usize = content.lines()
                                    .filter(|l| !l.starts_with("> "))
                                    .map(|l| l.len() + 1)
                                    .sum();
                                if user_text_len > char_limit + 1 {
                                    let private = RelayMessage::Private {
                                        to: my_key_for_recv.clone(),
                                        message: format!("Message too long ({} chars, max {}). Please shorten it.", user_text_len.saturating_sub(1), char_limit),
                                    };
                                    let _ = state_clone.broadcast_tx.send(private);
                                    continue;
                                }

                                // Handle slash commands (but not paths like /uploads/...).
                                let trimmed = content.trim();
                                if trimmed.starts_with('/') && !trimmed.starts_with("/uploads/") && !trimmed.contains('.') {
                                    let cmd = trimmed.split_whitespace().next().unwrap_or("").to_lowercase();
                                    match cmd.as_str() {
                                        "/link" => {
                                            // Generate a one-time device link code (private, only sender sees it).
                                            match state_clone.db.create_link_code(&display, &my_key_for_recv) {
                                                Ok(code) => {
                                                    let private = RelayMessage::Private {
                                                        to: my_key_for_recv.clone(),
                                                        message: format!(
                                                            "🔗 Link code: {}  — Enter this on your other device within 5 minutes. One-time use.",
                                                            code
                                                        ),
                                                    };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                }
                                                Err(e) => {
                                                    tracing::error!("Failed to create link code: {e}");
                                                }
                                            }
                                        }
                                        "/help" => {
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            let mut help_text = vec![
                                                "📖 Available commands:".to_string(),
                                                "  /help — Show this message".to_string(),
                                                "  /link — Generate a code to link another device".to_string(),
                                                "  /revoke <key_prefix> — Remove a stolen/lost device from your name".to_string(),
                                                "  /users — List all registered users (online/offline)".to_string(),
                                                "  /report <name> [reason] — Report a user".to_string(),
                                                "  /dm <name> <message> — Send a direct message".to_string(),
                                                "  /dms — List your DM conversations".to_string(),
                                                "  /edit <text> — Edit your last message".to_string(),
                                                "  /pins — List pinned messages".to_string(),
                                                "  /friend-code — Generate a friend code to share outside the platform".to_string(),
                                                "  /redeem <code> — Redeem a friend code (auto-mutual-follow)".to_string(),
                                                "  /server-list — List federated servers".to_string(),
                                            ];
                                            if role == "admin" || role == "mod" {
                                                help_text.push("".to_string());
                                                help_text.push("🛡️ Moderator commands:".to_string());
                                                help_text.push("  /kick <name> — Disconnect a user".to_string());
                                                help_text.push("  /mute <name> — Mute a user".to_string());
                                                help_text.push("  /unmute <name> — Unmute a user".to_string());
                                                help_text.push("  /pin — Pin the last message in the channel".to_string());
                                                help_text.push("  /unpin <N> — Unpin a message by its index".to_string());
                                            }
                                            if role == "admin" || role == "mod" {
                                                help_text.push("  /invite — Generate a one-time invite code for lockdown bypass".to_string());
                                            }
                                            if role == "admin" {
                                                help_text.push("".to_string());
                                                help_text.push("👑 Admin commands:".to_string());
                                                help_text.push("  /ban <name> — Ban a user".to_string());
                                                help_text.push("  /unban <name> — Unban a user".to_string());
                                                help_text.push("  /mod <name> — Make a user a moderator".to_string());
                                                help_text.push("  /unmod <name> — Remove moderator role".to_string());
                                                help_text.push("  /verify <name> — Mark a user as verified".to_string());
                                                help_text.push("  /donor <name> — Mark a user as a donor".to_string());
                                                help_text.push("  /unverify <name> — Remove verified status".to_string());
                                                help_text.push("  /lockdown — Toggle registration lockdown".to_string());
                                                help_text.push("  /invite — Generate invite code for lockdown bypass".to_string());
                                                help_text.push("  /wipe — Clear current channel's history".to_string());
                                                help_text.push("  /wipe-all — Clear ALL channels' history".to_string());
                                                help_text.push("  /gc — Garbage collect inactive names (90 days)".to_string());
                                                help_text.push("  /channel-create <name> [--readonly] [desc] — Create a channel".to_string());
                                                help_text.push("  /channel-delete <name> — Delete a channel".to_string());
                                                help_text.push("  /channel-readonly <name> — Toggle read-only on a channel".to_string());
                                                help_text.push("  /channel-reorder <name> <pos> — Set channel sort order (lower = higher)".to_string());
                                                help_text.push("  /name-release <name> — Release a name (for account recovery)".to_string());
                                                help_text.push("  /reports — View recent reports".to_string());
                                                help_text.push("  /reports-clear — Clear all reports".to_string());
                                                help_text.push("".to_string());
                                                help_text.push("🌐 Federation:".to_string());
                                                help_text.push("  /server-add <url> [name] — Add a federated server".to_string());
                                                help_text.push("  /server-remove <id> — Remove a federated server".to_string());
                                                help_text.push("  /server-trust <id> <0-3> — Set trust tier".to_string());
                                                help_text.push("  /server-federate <channel> — Toggle federation for a channel".to_string());
                                                help_text.push("  /server-connect — Connect to all verified federated servers".to_string());
                                            }
                                            help_text.push("".to_string());
                                            help_text.push("💡 Tips:".to_string());
                                            help_text.push("  • Click ↩ on any message to reply".to_string());
                                            help_text.push("  • **bold**, *italic*, `code`, ~~strike~~ for formatting".to_string());
                                            let private = RelayMessage::Private {
                                                to: my_key_for_recv.clone(),
                                                message: help_text.join("\n"),
                                            };
                                            let _ = state_clone.broadcast_tx.send(private);
                                        }
                                        "/channel-create" => {
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins can create channels.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                let ch_name = trimmed.split_whitespace().nth(1).unwrap_or("").to_lowercase();
                                                if ch_name.is_empty() || ch_name == "--readonly" || !ch_name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') || ch_name.len() > 24 {
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Usage: /channel-create <name> [--readonly] [description...]\nChannel name: 1-24 chars, letters/numbers/dashes/underscores.".to_string() };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                } else {
                                                    let args: Vec<&str> = trimmed.split_whitespace().skip(2).collect();
                                                    let read_only = args.iter().any(|a| *a == "--readonly");
                                                    let desc = args.iter().filter(|a| **a != "--readonly").copied().collect::<Vec<_>>().join(" ");
                                                    let desc_opt = if desc.is_empty() { None } else { Some(desc.as_str()) };
                                                    match state_clone.db.create_channel(&ch_name, &ch_name, desc_opt, &my_key_for_recv, read_only) {
                                                        Ok(true) => {
                                                            // Broadcast updated channel list to everyone.
                                                            broadcast_channel_list(&state_clone);
                                                            let ro_label = if read_only { " (read-only)" } else { "" };
                                                            let sys = RelayMessage::System { message: format!("Channel #{} created{}.", ch_name, ro_label) };
                                                            let _ = state_clone.broadcast_tx.send(sys);
                                                        }
                                                        Ok(false) => {
                                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Channel '{}' already exists.", ch_name) };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                        }
                                                        Err(e) => tracing::error!("Channel create error: {e}"),
                                                    }
                                                }
                                            }
                                        }
                                        "/channel-edit" => {
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins can edit channels.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                // Syntax: /channel-edit <old_name> name <new_name>
                                                let mut parts = trimmed.split_whitespace();
                                                let _cmd = parts.next();
                                                let old_name = parts
                                                    .next()
                                                    .unwrap_or("")
                                                    .trim()
                                                    .trim_start_matches('#')
                                                    .to_lowercase();
                                                let field = parts.next().unwrap_or("").trim();
                                                let new_name = parts.collect::<Vec<_>>().join(" ").trim().to_lowercase();

                                                if old_name.is_empty() || field != "name" || new_name.is_empty() {
                                                    let private = RelayMessage::Private {
                                                        to: my_key_for_recv.clone(),
                                                        message: "Usage: /channel-edit <old_name> name <new_name>".to_string(),
                                                    };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                } else if !new_name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') || new_name.len() > 24 {
                                                    let private = RelayMessage::Private {
                                                        to: my_key_for_recv.clone(),
                                                        message: "Invalid channel name. Use 1-24 chars: letters/numbers/dashes/underscores.".to_string(),
                                                    };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                } else if old_name == "general" {
                                                    let private = RelayMessage::Private {
                                                        to: my_key_for_recv.clone(),
                                                        message: "Cannot rename the general channel.".to_string(),
                                                    };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                } else {
                                                    match state_clone.db.rename_channel(&old_name, &new_name) {
                                                        Ok(true) => {
                                                            broadcast_channel_list(&state_clone);
                                                            let sys = RelayMessage::System {
                                                                message: format!("Channel #{} renamed to #{}.", old_name, new_name),
                                                            };
                                                            let _ = state_clone.broadcast_tx.send(sys);
                                                        }
                                                        Ok(false) => {
                                                            let private = RelayMessage::Private {
                                                                to: my_key_for_recv.clone(),
                                                                message: format!("Unable to rename channel '{}' (not found or destination exists).", old_name),
                                                            };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                        }
                                                        Err(e) => tracing::error!("Channel edit error: {e}"),
                                                    }
                                                }
                                            }
                                        }
                                        "/channel-delete" => {
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" && role != "mod" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins and mods can delete channels.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                let ch_name = trimmed
                                                    .split_whitespace()
                                                    .nth(1)
                                                    .unwrap_or("")
                                                    .trim()
                                                    .trim_start_matches('#')
                                                    .to_lowercase();
                                                if ch_name == "general" {
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Cannot delete the general channel.".to_string() };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                } else if state_clone.db.delete_channel(&ch_name).unwrap_or(false) {
                                                    broadcast_channel_list(&state_clone);
                                                    let sys = RelayMessage::System { message: format!("Channel #{} deleted.", ch_name) };
                                                    let _ = state_clone.broadcast_tx.send(sys);
                                                } else {
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Channel '{}' not found.", ch_name) };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                }
                                            }
                                        }
                                        "/channel-readonly" => {
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins can toggle read-only channels.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                let ch_name = trimmed.split_whitespace().nth(1).unwrap_or("").to_lowercase();
                                                if ch_name.is_empty() {
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Usage: /channel-readonly <name>".to_string() };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                } else {
                                                    let current_ro = state_clone.db.is_channel_read_only(&ch_name).unwrap_or(false);
                                                    let new_ro = !current_ro;
                                                    match state_clone.db.set_channel_read_only(&ch_name, new_ro) {
                                                        Ok(true) => {
                                                            broadcast_channel_list(&state_clone);
                                                            let status = if new_ro { "now read-only 🔒" } else { "now writable 🔓" };
                                                            let sys = RelayMessage::System { message: format!("Channel #{} is {}.", ch_name, status) };
                                                            let _ = state_clone.broadcast_tx.send(sys);
                                                        }
                                                        Ok(false) => {
                                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Channel '{}' not found.", ch_name) };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                        }
                                                        Err(e) => tracing::error!("Channel readonly toggle error: {e}"),
                                                    }
                                                }
                                            }
                                        }
                                        "/channel-reorder" => {
                                            // Usage: /channel-reorder <name> <position>
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins can reorder channels.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                                                if parts.len() < 3 {
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Usage: /channel-reorder <name> <position>\nLower numbers appear first (e.g., 0, 1, 2, 10, 20).".to_string() };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                } else {
                                                    let ch_name = parts[1];
                                                    if let Ok(pos) = parts[2].parse::<i32>() {
                                                        if state_clone.db.set_channel_position(ch_name, pos).unwrap_or(false) {
                                                            // Broadcast updated list.
                                                            broadcast_channel_list(&state_clone);
                                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Channel #{} moved to position {}.", ch_name, pos) };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                        } else {
                                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Channel '{}' not found.", ch_name) };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                        }
                                                    } else {
                                                        let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Position must be a number.".to_string() };
                                                        let _ = state_clone.broadcast_tx.send(private);
                                                    }
                                                }
                                            }
                                        }
                                        "/invite" => {
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" && role != "mod" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins and mods can generate invite codes.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                match state_clone.db.create_invite_code(&my_key_for_recv) {
                                                    Ok(code) => {
                                                        let private = RelayMessage::Private {
                                                            to: my_key_for_recv.clone(),
                                                            message: format!("🎫 Invite code: {}  — Share this with someone to let them register during lockdown. Valid for 24 hours, one-time use.", code),
                                                        };
                                                        let _ = state_clone.broadcast_tx.send(private);
                                                    }
                                                    Err(e) => {
                                                        tracing::error!("Failed to create invite code: {e}");
                                                    }
                                                }
                                            }
                                        }
                                        "/revoke" => {
                                            // Revoke a device from your own name. Usage: /revoke <key_prefix>
                                            let prefix = trimmed.split_whitespace().nth(1).unwrap_or("");
                                            if prefix.len() < 6 {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Usage: /revoke <first 6+ chars of device key>. Check your devices in the sidebar.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else if prefix.starts_with(&my_key_for_recv[..6]) {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "You can't revoke your current device. Use another linked device.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                match state_clone.db.revoke_device(&display, prefix) {
                                                    Ok(keys) if !keys.is_empty() => {
                                                        let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Revoked {} device(s) from your name.", keys.len()) };
                                                        let _ = state_clone.broadcast_tx.send(private);
                                                    }
                                                    Ok(_) => {
                                                        let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("No devices found matching prefix '{}'.", prefix) };
                                                        let _ = state_clone.broadcast_tx.send(private);
                                                    }
                                                    Err(e) => tracing::error!("Revoke error: {e}"),
                                                }
                                            }
                                        }
                                        "/name-release" => {
                                            // Admin-only: release a name entirely so it can be re-registered.
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins can release names.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                let target = trimmed.split_whitespace().nth(1).unwrap_or("");
                                                if target.is_empty() {
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Usage: /name-release <name>".to_string() };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                } else {
                                                    match state_clone.db.release_name(target) {
                                                        Ok(n) if n > 0 => {
                                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Released name '{}' ({} key bindings removed). It can now be claimed by anyone.", target, n) };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                        }
                                                        Ok(_) => {
                                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Name '{}' not found.", target) };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                        }
                                                        Err(e) => tracing::error!("Name release error: {e}"),
                                                    }
                                                }
                                            }
                                        }
                                        "/lockdown" => {
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins can toggle lockdown.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                let mut locked = state_clone.lockdown.write().await;
                                                *locked = !*locked;
                                                // Manual lockdown: clear auto_lockdown flag.
                                                *state_clone.auto_lockdown.write().await = false;
                                                // L-4: Persist lockdown state.
                                                let _ = state_clone.db.set_state("lockdown", if *locked { "true" } else { "false" });
                                                let msg = if *locked {
                                                    "🔒 Registration locked"
                                                } else {
                                                    "🔓 Registration opened"
                                                };
                                                let sys = RelayMessage::System { message: msg.to_string() };
                                                state_clone.broadcast_and_store(sys).await;
                                            }
                                        }
                                        "/verify" => {
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins can verify users.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                let target_name = trimmed.split_whitespace().nth(1).unwrap_or("").to_string();
                                                if target_name.is_empty() {
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Usage: /verify <name>".to_string() };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                } else {
                                                    match state_clone.db.keys_for_name(&target_name) {
                                                        Ok(keys) if !keys.is_empty() => {
                                                            for key in &keys {
                                                                if let Err(e) = state_clone.db.set_role(key, "verified") {
                                                                    tracing::error!("Failed to verify: {e}");
                                                                }
                                                            }
                                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("✦ {} is now verified.", target_name) };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                            // Broadcast updated peer list so badges refresh.
                                                            broadcast_peer_list(&state_clone).await;
                                                            broadcast_full_user_list(&state_clone).await;
                                                        }
                                                        _ => {
                                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("User '{}' not found.", target_name) };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        "/donor" => {
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins can set donor status.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                let target_name = trimmed.split_whitespace().nth(1).unwrap_or("").to_string();
                                                if target_name.is_empty() {
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Usage: /donor <name>".to_string() };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                } else {
                                                    match state_clone.db.keys_for_name(&target_name) {
                                                        Ok(keys) if !keys.is_empty() => {
                                                            for key in &keys {
                                                                if let Err(e) = state_clone.db.set_role(key, "donor") {
                                                                    tracing::error!("Failed to set donor: {e}");
                                                                }
                                                            }
                                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("💎 {} is now a donor. Thank you!", target_name) };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                            broadcast_peer_list(&state_clone).await;
                                                            broadcast_full_user_list(&state_clone).await;
                                                        }
                                                        _ => {
                                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("User '{}' not found.", target_name) };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        "/unverify" => {
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins can unverify users.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                let target_name = trimmed.split_whitespace().nth(1).unwrap_or("").to_string();
                                                if target_name.is_empty() {
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Usage: /unverify <name>".to_string() };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                } else {
                                                    match state_clone.db.keys_for_name(&target_name) {
                                                        Ok(keys) if !keys.is_empty() => {
                                                            for key in &keys {
                                                                if let Err(e) = state_clone.db.set_role(key, "user") {
                                                                    tracing::error!("Failed to unverify: {e}");
                                                                }
                                                            }
                                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("{} is no longer verified.", target_name) };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                            broadcast_peer_list(&state_clone).await;
                                                            broadcast_full_user_list(&state_clone).await;
                                                        }
                                                        _ => {
                                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("User '{}' not found.", target_name) };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        "/wipe" => {
                                            // Wipes messages in the CURRENT channel only.
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins can wipe messages.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                let wipe_ch = if channel.is_empty() { "general".to_string() } else { channel.clone() };
                                                match state_clone.db.wipe_channel_messages(&wipe_ch) {
                                                    Ok(count) => {
                                                        // Clear in-memory history for this channel.
                                                        {
                                                            let mut history = state_clone.history.write().await;
                                                            history.retain(|m| {
                                                                if let RelayMessage::Chat { channel: ch, .. } = m {
                                                                    ch != &wipe_ch
                                                                } else {
                                                                    true
                                                                }
                                                            });
                                                        }
                                                        let sys = RelayMessage::System {
                                                            message: format!("💥 #{} history cleared by admin ({} messages).", wipe_ch, count),
                                                        };
                                                        let _ = state_clone.broadcast_tx.send(sys);
                                                        info!("Admin wiped {} messages from #{}", count, wipe_ch);
                                                    }
                                                    Err(e) => {
                                                        tracing::error!("Wipe failed: {e}");
                                                        let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Wipe failed: {e}") };
                                                        let _ = state_clone.broadcast_tx.send(private);
                                                    }
                                                }
                                            }
                                        }
                                        "/wipe-all" => {
                                            // Nuclear option: wipes ALL channels.
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins can wipe messages.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                match state_clone.db.wipe_messages() {
                                                    Ok(count) => {
                                                        state_clone.history.write().await.clear();
                                                        let sys = RelayMessage::System {
                                                            message: format!("💥 All chat history cleared by admin ({} messages).", count),
                                                        };
                                                        let _ = state_clone.broadcast_tx.send(sys);
                                                        info!("Admin wiped ALL {} messages", count);
                                                    }
                                                    Err(e) => {
                                                        tracing::error!("Wipe-all failed: {e}");
                                                        let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Wipe failed: {e}") };
                                                        let _ = state_clone.broadcast_tx.send(private);
                                                    }
                                                }
                                            }
                                        }
                                        "/users" => {
                                            {
                                                match state_clone.db.list_all_users() {
                                                    Ok(users) => {
                                                        let online_names: std::collections::HashSet<String> = state_clone.peers.read().await.values()
                                                            .filter_map(|p| p.display_name.clone())
                                                            .map(|n| n.to_lowercase())
                                                            .collect();

                                                        let mut lines = vec![format!("👥 Registered users ({}):", users.len())];
                                                        for (name, role, key_count) in &users {
                                                            let is_online = online_names.contains(&name.to_lowercase());
                                                            let status = if is_online { "🟢" } else { "⚫" };
                                                            let role_badge = match role.as_str() {
                                                                "admin" => " 👑",
                                                                "mod" => " 🛡️",
                                                                "verified" => " ✦",
                                                                "donor" => " 💎",
                                                                "muted" => " 🔇",
                                                                _ => "",
                                                            };
                                                            let devices = if *key_count > 1 { format!(" ({} devices)", key_count) } else { String::new() };
                                                            lines.push(format!("  {} {}{}{}", status, name, role_badge, devices));
                                                        }
                                                        let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: lines.join("\n") };
                                                        let _ = state_clone.broadcast_tx.send(private);
                                                    }
                                                    Err(e) => {
                                                        tracing::error!("Failed to list users: {e}");
                                                        let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Error: {e}") };
                                                        let _ = state_clone.broadcast_tx.send(private);
                                                    }
                                                }
                                            }
                                        }
                                        "/gc" => {
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins can run garbage collection.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                match state_clone.db.garbage_collect_names(90) {
                                                    Ok(deleted) if deleted.is_empty() => {
                                                        let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "🧹 No inactive names to clean up.".to_string() };
                                                        let _ = state_clone.broadcast_tx.send(private);
                                                    }
                                                    Ok(deleted) => {
                                                        let names_list = deleted.join(", ");
                                                        let private = RelayMessage::Private {
                                                            to: my_key_for_recv.clone(),
                                                            message: format!("🧹 Garbage collected {} inactive name(s): {}", deleted.len(), names_list),
                                                        };
                                                        let _ = state_clone.broadcast_tx.send(private);
                                                    }
                                                    Err(e) => {
                                                        tracing::error!("GC failed: {e}");
                                                        let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("GC failed: {e}") };
                                                        let _ = state_clone.broadcast_tx.send(private);
                                                    }
                                                }
                                            }
                                        }
                                        // ── Moderation commands ──
                                        "/kick" | "/ban" | "/unban" | "/mod" | "/unmod" | "/mute" | "/unmute" => {
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            let target_name = trimmed.split_whitespace().nth(1).unwrap_or("").to_string();

                                            if target_name.is_empty() {
                                                let private = RelayMessage::Private {
                                                    to: my_key_for_recv.clone(),
                                                    message: format!("Usage: {} <name>", cmd),
                                                };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                let result = handle_mod_command(
                                                    &state_clone, &cmd, &role, &target_name, &my_key_for_recv
                                                ).await;
                                                let private = RelayMessage::Private {
                                                    to: my_key_for_recv.clone(),
                                                    message: result,
                                                };
                                                let _ = state_clone.broadcast_tx.send(private);
                                                // Refresh peer list after role/status changes.
                                                broadcast_peer_list(&state_clone).await;
                                                broadcast_full_user_list(&state_clone).await;
                                            }
                                        }
                                        "/report" => {
                                            // /report <name> [reason] — available to all users.
                                            let parts: Vec<&str> = trimmed.splitn(3, char::is_whitespace).collect();
                                            let target_name = parts.get(1).unwrap_or(&"").to_string();
                                            let reason = parts.get(2).unwrap_or(&"").to_string();
                                            if target_name.is_empty() {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Usage: /report <name> [reason]".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else if target_name.eq_ignore_ascii_case(&display) {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "You can't report yourself.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                // Check target exists.
                                                match state_clone.db.keys_for_name(&target_name) {
                                                    Ok(keys) if !keys.is_empty() => {
                                                        // Rate limit check.
                                                        let now_ms = std::time::SystemTime::now()
                                                            .duration_since(std::time::UNIX_EPOCH)
                                                            .unwrap_or_default()
                                                            .as_millis() as i64;
                                                        let one_hour_ago = now_ms - 3_600_000;
                                                        let recent_count = state_clone.db.count_recent_reports(&my_key_for_recv, one_hour_ago).unwrap_or(0);
                                                        let reporter_role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                                        let max_reports: usize = if reporter_role == "verified" || reporter_role == "donor" { 5 } else { 3 };
                                                        if recent_count >= max_reports {
                                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("You've reached the report limit ({} per hour). Please wait.", max_reports) };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                        } else {
                                                            // Store report.
                                                            if let Err(e) = state_clone.db.add_report(&my_key_for_recv, &target_name, &reason) {
                                                                tracing::error!("Failed to add report: {e}");
                                                            }
                                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("✅ Report submitted for {}.", target_name) };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                            // Notify all online admins.
                                                            let reason_display = if reason.is_empty() { "(no reason)".to_string() } else { reason.clone() };
                                                            let peers = state_clone.peers.read().await;
                                                            for p in peers.values() {
                                                                let pr = state_clone.db.get_role(&p.public_key_hex).unwrap_or_default();
                                                                if pr == "admin" || pr == "mod" {
                                                                    let notif = RelayMessage::Private {
                                                                        to: p.public_key_hex.clone(),
                                                                        message: format!("⚠️ New report: {} reported {} — {}", display, target_name, reason_display),
                                                                    };
                                                                    let _ = state_clone.broadcast_tx.send(notif);
                                                                }
                                                            }
                                                        }
                                                    }
                                                    _ => {
                                                        let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("User '{}' not found.", target_name) };
                                                        let _ = state_clone.broadcast_tx.send(private);
                                                    }
                                                }
                                            }
                                        }
                                        "/reports" => {
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" && role != "mod" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins and mods can view reports.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                match state_clone.db.get_reports(20) {
                                                    Ok(reports) if reports.is_empty() => {
                                                        let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "No reports.".to_string() };
                                                        let _ = state_clone.broadcast_tx.send(private);
                                                    }
                                                    Ok(reports) => {
                                                        let now_ms = std::time::SystemTime::now()
                                                            .duration_since(std::time::UNIX_EPOCH)
                                                            .unwrap_or_default()
                                                            .as_millis() as i64;
                                                        let mut lines = vec!["📋 Recent reports:".to_string()];
                                                        for (id, reporter_key, reported_name, reason, created_at) in &reports {
                                                            let ago_secs = ((now_ms - created_at) / 1000).max(0);
                                                            let time_ago = if ago_secs < 60 { format!("{}s ago", ago_secs) }
                                                                else if ago_secs < 3600 { format!("{}m ago", ago_secs / 60) }
                                                                else if ago_secs < 86400 { format!("{}h ago", ago_secs / 3600) }
                                                                else { format!("{}d ago", ago_secs / 86400) };
                                                            let reporter_short = if reporter_key.len() > 8 { &reporter_key[..8] } else { reporter_key };
                                                            let reason_display = if reason.is_empty() { "(no reason)" } else { reason.as_str() };
                                                            lines.push(format!("  {} | {}… → {} | {} | {}", id, reporter_short, reported_name, reason_display, time_ago));
                                                        }
                                                        let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: lines.join("\n") };
                                                        let _ = state_clone.broadcast_tx.send(private);
                                                    }
                                                    Err(e) => {
                                                        tracing::error!("Failed to get reports: {e}");
                                                        let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Error: {e}") };
                                                        let _ = state_clone.broadcast_tx.send(private);
                                                    }
                                                }
                                            }
                                        }
                                        "/reports-clear" => {
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins can clear reports.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                match state_clone.db.clear_reports() {
                                                    Ok(count) => {
                                                        let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("🧹 Cleared {} report(s).", count) };
                                                        let _ = state_clone.broadcast_tx.send(private);
                                                    }
                                                    Err(e) => {
                                                        tracing::error!("Failed to clear reports: {e}");
                                                        let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Error: {e}") };
                                                        let _ = state_clone.broadcast_tx.send(private);
                                                    }
                                                }
                                            }
                                        }
                                        "/dm" => {
                                            // /dm <name> <message> — Send a DM.
                                            let parts: Vec<&str> = trimmed.splitn(3, char::is_whitespace).collect();
                                            let target_name = parts.get(1).unwrap_or(&"").to_string();
                                            let dm_content = parts.get(2).unwrap_or(&"").to_string();
                                            if target_name.is_empty() || dm_content.is_empty() {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Usage: /dm <name> <message>".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else if target_name.eq_ignore_ascii_case(&display) {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "You can't DM yourself.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else if dm_content.len() > if user_role == "admin" { 10_000 } else { 2_000 } {
                                                let limit = if user_role == "admin" { 10_000 } else { 2_000 };
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("DM too long (max {} chars).", limit) };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                match state_clone.db.keys_for_name(&target_name) {
                                                    Ok(keys) if !keys.is_empty() => {
                                                        let target_key = keys[0].clone();
                                                        let ts = std::time::SystemTime::now()
                                                            .duration_since(std::time::UNIX_EPOCH)
                                                            .unwrap_or_default()
                                                            .as_millis() as u64;
                                                        if let Err(e) = state_clone.db.store_dm(&my_key_for_recv, &display, &target_key, &dm_content, ts) {
                                                            tracing::error!("Failed to store DM: {e}");
                                                        }
                                                        // Send to recipient (/dm command sends plaintext).
                                                        let dm_msg = RelayMessage::Dm {
                                                            from: my_key_for_recv.clone(),
                                                            from_name: Some(display.clone()),
                                                            to: target_key.clone(),
                                                            content: dm_content.clone(),
                                                            timestamp: ts,
                                                            encrypted: false,
                                                            nonce: None,
                                                        };
                                                        let _ = state_clone.broadcast_tx.send(dm_msg);
                                                        // Send DM list update to both parties.
                                                        send_dm_list_update(&state_clone, &my_key_for_recv);
                                                        send_dm_list_update(&state_clone, &target_key);
                                                        let private = RelayMessage::Private {
                                                            to: my_key_for_recv.clone(),
                                                            message: format!("💬 DM sent to {}.", target_name),
                                                        };
                                                        let _ = state_clone.broadcast_tx.send(private);
                                                    }
                                                    _ => {
                                                        let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("User '{}' not found.", target_name) };
                                                        let _ = state_clone.broadcast_tx.send(private);
                                                    }
                                                }
                                            }
                                        }
                                        "/dms" => {
                                            // List DM conversations.
                                            send_dm_list_update(&state_clone, &my_key_for_recv);
                                        }
                                        "/pin" => {
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" && role != "mod" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins and mods can pin messages.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                let ch = if channel.is_empty() { "general".to_string() } else { channel.clone() };
                                                match state_clone.db.get_last_message_in_channel(&ch) {
                                                    Ok(Some((from_key, from_name, content, ts))) => {
                                                        match state_clone.db.pin_message(&ch, &from_key, &from_name, &content, ts, &display) {
                                                            Ok(true) => {
                                                                let pin = PinData {
                                                                    from_key,
                                                                    from_name: from_name.clone(),
                                                                    content: content.clone(),
                                                                    original_timestamp: ts,
                                                                    pinned_by: display.clone(),
                                                                    pinned_at: std::time::SystemTime::now()
                                                                        .duration_since(std::time::UNIX_EPOCH)
                                                                        .unwrap_or_default()
                                                                        .as_millis() as u64,
                                                                };
                                                                let _ = state_clone.broadcast_tx.send(RelayMessage::PinAdded {
                                                                    channel: ch.clone(),
                                                                    pin,
                                                                });
                                                                let sys = RelayMessage::System {
                                                                    message: format!("📌 {} pinned a message by {}.", display, from_name),
                                                                };
                                                                let _ = state_clone.broadcast_tx.send(sys);
                                                            }
                                                            Ok(false) => {
                                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "That message is already pinned.".to_string() };
                                                                let _ = state_clone.broadcast_tx.send(private);
                                                            }
                                                            Err(e) => {
                                                                tracing::error!("Pin error: {e}");
                                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Pin failed: {e}") };
                                                                let _ = state_clone.broadcast_tx.send(private);
                                                            }
                                                        }
                                                    }
                                                    Ok(None) => {
                                                        let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "No messages to pin in this channel.".to_string() };
                                                        let _ = state_clone.broadcast_tx.send(private);
                                                    }
                                                    Err(e) => {
                                                        tracing::error!("Pin lookup error: {e}");
                                                    }
                                                }
                                            }
                                        }
                                        "/unpin" => {
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" && role != "mod" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins and mods can unpin messages.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                let idx_str = trimmed.split_whitespace().nth(1).unwrap_or("0");
                                                let idx: usize = idx_str.parse().unwrap_or(0);
                                                if idx == 0 {
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Usage: /unpin <number> — use /pins to see the list.".to_string() };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                } else {
                                                    let ch = if channel.is_empty() { "general".to_string() } else { channel.clone() };
                                                    match state_clone.db.unpin_message(&ch, idx) {
                                                        Ok(true) => {
                                                            let _ = state_clone.broadcast_tx.send(RelayMessage::PinRemoved {
                                                                channel: ch.clone(),
                                                                index: idx,
                                                            });
                                                            let sys = RelayMessage::System {
                                                                message: format!("📌 {} unpinned message #{}.", display, idx),
                                                            };
                                                            let _ = state_clone.broadcast_tx.send(sys);
                                                        }
                                                        Ok(false) => {
                                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Pin #{} not found. Use /pins to see the list.", idx) };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                        }
                                                        Err(e) => {
                                                            tracing::error!("Unpin error: {e}");
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        "/pins" => {
                                            let ch = if channel.is_empty() { "general".to_string() } else { channel.clone() };
                                            match state_clone.db.get_pinned_messages(&ch) {
                                                Ok(pins) if pins.is_empty() => {
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("No pinned messages in #{}.", ch) };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                }
                                                Ok(pins) => {
                                                    let mut lines = vec![format!("📌 Pinned messages in #{} ({}):", ch, pins.len())];
                                                    for (i, pin) in pins.iter().enumerate() {
                                                        let short_content = if pin.content.len() > 80 {
                                                            format!("{}…", &pin.content[..80])
                                                        } else {
                                                            pin.content.clone()
                                                        };
                                                        lines.push(format!("  {}. {} — \"{}\"", i + 1, pin.from_name, short_content));
                                                    }
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: lines.join("\n") };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                }
                                                Err(e) => {
                                                    tracing::error!("Pins list error: {e}");
                                                }
                                            }
                                        }
                                        "/edit" => {
                                            // /edit <new content> — edit your last message in this channel.
                                            let new_content = trimmed.strip_prefix("/edit").unwrap_or("").trim().to_string();
                                            if new_content.is_empty() {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Usage: /edit <new message text>".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else if new_content.len() > if user_role == "admin" { 10_000 } else { 2_000 } {
                                                let limit = if user_role == "admin" { 10_000 } else { 2_000 };
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Message too long (max {} chars).", limit) };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                // Find user's last message in this channel.
                                                let ch = if channel.is_empty() { "general".to_string() } else { channel.clone() };
                                                let last_ts = state_clone.db.get_last_user_message_timestamp(&my_key_for_recv, &ch).unwrap_or(None);
                                                if let Some(ts) = last_ts {
                                                    match state_clone.db.edit_message(&my_key_for_recv, ts, &new_content) {
                                                        Ok(true) => {
                                                            let edit = RelayMessage::Edit {
                                                                from: my_key_for_recv.clone(),
                                                                timestamp: ts,
                                                                new_content: new_content.clone(),
                                                                channel: ch,
                                                            };
                                                            let _ = state_clone.broadcast_tx.send(edit);
                                                        }
                                                        Ok(false) => {
                                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Could not find your message to edit.".to_string() };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                        }
                                                        Err(e) => {
                                                            tracing::error!("Edit error: {e}");
                                                        }
                                                    }
                                                } else {
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "No messages found to edit.".to_string() };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                }
                                            }
                                        }
                                        // ── Federation commands ──
                                        "/server-add" => {
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins can add federated servers.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                let parts: Vec<&str> = trimmed.splitn(3, char::is_whitespace).collect();
                                                let url = parts.get(1).unwrap_or(&"").to_string();
                                                let name = parts.get(2).map(|s| s.to_string());
                                                if url.is_empty() || (!url.starts_with("http://") && !url.starts_with("https://")) {
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Usage: /server-add <url> [name]\nURL must start with http:// or https://".to_string() };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                } else {
                                                    // Use URL as server_id for now (unique).
                                                    let server_id = url.trim_end_matches('/').to_string();
                                                    let display_name = name.unwrap_or_else(|| server_id.clone());
                                                    match state_clone.db.add_federated_server(&server_id, &display_name, &url) {
                                                        Ok(true) => {
                                                            // Try to fetch server-info to populate details.
                                                            let info_url = format!("{}/api/server-info", url.trim_end_matches('/'));
                                                            let sid = server_id.clone();
                                                            // Fire and forget — don't block on discovery.
                                                            let state_for_discover = state_clone.clone();
                                                            tokio::spawn(async move {
                                                                match state_for_discover.http_client.get(&info_url).timeout(std::time::Duration::from_secs(10)).send().await {
                                                                    Ok(resp) if resp.status().is_success() => {
                                                                        if let Ok(info) = resp.json::<serde_json::Value>().await {
                                                                            let name = info["name"].as_str().unwrap_or("Unknown");
                                                                            let pk = info["public_key"].as_str();
                                                                            let accord = info["accord_compliant"].as_bool().unwrap_or(false);
                                                                            let _ = state_for_discover.db.update_federated_server_info(&sid, name, pk, accord);
                                                                            tracing::info!("Discovered federated server: {} ({})", name, sid);
                                                                        }
                                                                    }
                                                                    Ok(resp) => {
                                                                        let _ = state_for_discover.db.update_federated_server_status(&sid, "unreachable");
                                                                        tracing::warn!("Server-info fetch failed for {}: HTTP {}", sid, resp.status());
                                                                    }
                                                                    Err(e) => {
                                                                        let _ = state_for_discover.db.update_federated_server_status(&sid, "unreachable");
                                                                        tracing::warn!("Server-info fetch failed for {}: {}", sid, e);
                                                                    }
                                                                }
                                                            });
                                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("✅ Added federated server: {}", display_name) };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                        }
                                                        Ok(false) => {
                                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Server already in registry.".to_string() };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                        }
                                                        Err(e) => {
                                                            tracing::error!("Failed to add server: {e}");
                                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Error: {e}") };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        "/server-remove" => {
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins can remove federated servers.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                let server_id = trimmed.split_whitespace().nth(1).unwrap_or("").to_string();
                                                if server_id.is_empty() {
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Usage: /server-remove <server_id or url>".to_string() };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                } else {
                                                    match state_clone.db.remove_federated_server(&server_id) {
                                                        Ok(true) => {
                                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Removed federated server: {}", server_id) };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                        }
                                                        Ok(false) => {
                                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Server not found in registry.".to_string() };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                        }
                                                        Err(e) => {
                                                            tracing::error!("Failed to remove server: {e}");
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        "/server-list" => {
                                            match state_clone.db.list_federated_servers() {
                                                Ok(servers) if servers.is_empty() => {
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "No federated servers registered.".to_string() };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                }
                                                Ok(servers) => {
                                                    let mut lines = vec![format!("🌐 Federated servers ({}):", servers.len())];
                                                    for s in &servers {
                                                        let tier_badge = match s.trust_tier {
                                                            3 => "🟢",
                                                            2 => "🟡",
                                                            1 => "🔵",
                                                            _ => "⚪",
                                                        };
                                                        let status_icon = match s.status.as_str() {
                                                            "online" => "●",
                                                            "unreachable" => "○",
                                                            _ => "?",
                                                        };
                                                        lines.push(format!("  {} {} {} — {} [T{}]", status_icon, tier_badge, s.name, s.url, s.trust_tier));
                                                    }
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: lines.join("\n") };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                }
                                                Err(e) => {
                                                    tracing::error!("Failed to list servers: {e}");
                                                }
                                            }
                                        }
                                        "/server-trust" => {
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins can set trust tiers.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                                                if parts.len() < 3 {
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Usage: /server-trust <server_id> <0-3>\nTiers: 3=Verified+Accord, 2=Verified, 1=Accord, 0=Unverified".to_string() };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                } else {
                                                    let server_id = parts[1];
                                                    if let Ok(tier) = parts[2].parse::<i32>() {
                                                        if !(0..=3).contains(&tier) {
                                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Trust tier must be 0-3.".to_string() };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                        } else if state_clone.db.set_server_trust_tier(server_id, tier).unwrap_or(false) {
                                                            let tier_label = match tier {
                                                                3 => "Verified + Accord 🟢",
                                                                2 => "Verified 🟡",
                                                                1 => "Unverified + Accord 🔵",
                                                                _ => "Unverified ⚪",
                                                            };
                                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Set {} to tier {} ({})", server_id, tier, tier_label) };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                        } else {
                                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Server not found in registry.".to_string() };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                        }
                                                    } else {
                                                        let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Tier must be a number (0-3).".to_string() };
                                                        let _ = state_clone.broadcast_tx.send(private);
                                                    }
                                                }
                                            }
                                        }
                                        "/server-federate" => {
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins can federate channels.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                let channel_name = trimmed.split_whitespace().nth(1).unwrap_or("").to_string();
                                                if channel_name.is_empty() {
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Usage: /server-federate <channel_name>\nToggles federation for a channel.".to_string() };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                } else {
                                                    let currently_federated = state_clone.db.is_channel_federated(&channel_name).unwrap_or(false);
                                                    let new_val = !currently_federated;
                                                    match state_clone.db.set_channel_federated(&channel_name, new_val) {
                                                        Ok(true) => {
                                                            let icon = if new_val { "🌐" } else { "🔒" };
                                                            let action = if new_val { "federated" } else { "un-federated" };
                                                            let sys = RelayMessage::System { message: format!("{} Channel '{}' is now {}.", icon, channel_name, action) };
                                                            let _ = state_clone.broadcast_tx.send(sys);
                                                            broadcast_channel_list(&state_clone);
                                                        }
                                                        Ok(false) => {
                                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Channel '{}' not found.", channel_name) };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                        }
                                                        Err(e) => {
                                                            tracing::error!("Failed to set channel federation: {e}");
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        "/server-connect" => {
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins can initiate federation connections.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                let fed_state = state_clone.clone();
                                                let my_key = my_key_for_recv.clone();
                                                tokio::spawn(async move {
                                                    let count = start_federation_connections(&fed_state).await;
                                                    let private = RelayMessage::Private { to: my_key, message: format!("🌐 Federation: initiated connections to {} servers.", count) };
                                                    let _ = fed_state.broadcast_tx.send(private);
                                                    broadcast_federation_status(&fed_state).await;
                                                });
                                            }
                                        }
                                        "/category-create" => {
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins can create categories.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                let cat_name = trimmed.split_whitespace().nth(1).unwrap_or("").to_string();
                                                if cat_name.is_empty() {
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Usage: /category-create <name>".to_string() };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                } else {
                                                    match state_clone.db.create_category(&cat_name, 100) {
                                                        Ok(_id) => {
                                                            broadcast_channel_list(&state_clone);
                                                            let sys = RelayMessage::System { message: format!("📁 Category '{}' created.", cat_name) };
                                                            let _ = state_clone.broadcast_tx.send(sys);
                                                        }
                                                        Err(e) => {
                                                            tracing::error!("Category create error: {e}");
                                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Error: {e}") };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        "/category-delete" => {
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins can delete categories.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                let cat_name = trimmed.split_whitespace().nth(1).unwrap_or("").to_string();
                                                if cat_name.is_empty() {
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Usage: /category-delete <name>".to_string() };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                } else if state_clone.db.delete_category(&cat_name).unwrap_or(false) {
                                                    broadcast_channel_list(&state_clone);
                                                    let sys = RelayMessage::System { message: format!("📁 Category '{}' deleted.", cat_name) };
                                                    let _ = state_clone.broadcast_tx.send(sys);
                                                } else {
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Category '{}' not found.", cat_name) };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                }
                                            }
                                        }
                                        "/category-rename" => {
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins can rename categories.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                                                if parts.len() < 3 {
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Usage: /category-rename <old> <new>".to_string() };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                } else if state_clone.db.rename_category(parts[1], parts[2]).unwrap_or(false) {
                                                    broadcast_channel_list(&state_clone);
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Renamed category '{}' to '{}'.", parts[1], parts[2]) };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                } else {
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Category '{}' not found.", parts[1]) };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                }
                                            }
                                        }
                                        "/channel-category" => {
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins can set channel categories.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                                                if parts.len() < 3 {
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Usage: /channel-category <channel> <category>".to_string() };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                } else if state_clone.db.set_channel_category(parts[1], parts[2]).unwrap_or(false) {
                                                    broadcast_channel_list(&state_clone);
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Moved #{} to category '{}'.", parts[1], parts[2]) };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                } else {
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Channel '{}' or category '{}' not found.", parts[1], parts[2]) };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                }
                                            }
                                        }
                                        // Silently ignore client-side-only commands
                                        "/groups" | "/dms" | "/servers" | "/friends" => {}
                                        _ => {
                                            let private = RelayMessage::Private {
                                                to: my_key_for_recv.clone(),
                                                message: format!("Unknown command: {}. Type /help for available commands.", cmd),
                                            };
                                            let _ = state_clone.broadcast_tx.send(private);
                                        }
                                    }
                                    continue; // Commands are never broadcast as chat.
                                }

                                let ch = if channel.is_empty() { "general".to_string() } else { channel };

                                // Check read-only channel.
                                if state_clone.db.is_channel_read_only(&ch).unwrap_or(false) {
                                    if user_role != "admin" && user_role != "mod" {
                                        let private = RelayMessage::Private {
                                            to: my_key_for_recv.clone(),
                                            message: "This channel is read-only.".to_string(),
                                        };
                                        let _ = state_clone.broadcast_tx.send(private);
                                        continue;
                                    }
                                }

                                // H-1: Server-side Ed25519 signature verification.
                                let verified_sig = if let Some(ref sig_hex) = signature {
                                    verify_ed25519_signature(&my_key_for_recv, &content, timestamp, sig_hex)
                                } else {
                                    false
                                };
                                // Only include signature in broadcast if it verified server-side.
                                let broadcast_sig = if verified_sig { signature } else { None };

                                let mut chat = RelayMessage::Chat {
                                    from: my_key_for_recv.clone(),
                                    from_name: Some(display.clone()),
                                    content: content.clone(),
                                    timestamp,
                                    signature: broadcast_sig,
                                    channel: ch.clone(),
                                    reply_to: reply_to.clone(),
                                    thread_count: None,
                                    message_id: None,
                                };

                                // Store in channel-specific table (with reply_to metadata).
                                // Capture the row ID so clients can correlate message_deleted events.
                                let stored_id = if let Some(ref rt) = reply_to {
                                    match state_clone.db.store_message_in_channel_with_reply(&chat, &ch, &rt.from, rt.timestamp) {
                                        Ok(id) => Some(id),
                                        Err(e) => { tracing::error!("Failed to persist message: {e}"); None }
                                    }
                                } else {
                                    match state_clone.db.store_message_in_channel(&chat, &ch) {
                                        Ok(id) => Some(id),
                                        Err(e) => { tracing::error!("Failed to persist message: {e}"); None }
                                    }
                                };
                                if let RelayMessage::Chat { ref mut message_id, .. } = chat {
                                    *message_id = stored_id;
                                }
                                // Broadcast to all (clients filter by their active channel).
                                let _ = state_clone.broadcast_tx.send(chat);

                                // Notify webhook for human messages (non-bot keys).
                                if !my_key_for_recv.starts_with("bot_") {
                                    state_clone.notify_webhook(&display, &content);
                                }

                                // Push notifications for @mentioned users who are offline.
                                {
                                    let mention_re = regex::Regex::new(r"@(\w+)").unwrap_or_else(|_| regex::Regex::new(r"$^").unwrap());
                                    let peers = state_clone.peers.read().await;
                                    for cap in mention_re.captures_iter(&content) {
                                        let mentioned_name = &cap[1];
                                        // Resolve name → public key(s), check if offline.
                                        if let Ok(keys) = state_clone.db.keys_for_name(mentioned_name) {
                                            for mentioned_key in keys {
                                                if mentioned_key != my_key_for_recv && !peers.contains_key(&mentioned_key) {
                                                    let preview = if content.len() > 100 { &content[..100] } else { &content };
                                                    state_clone.send_push_notification(
                                                        &mentioned_key,
                                                        &format!("{} mentioned you", display),
                                                        preview,
                                                        &format!("mention-{}", &ch),
                                                        "/chat",
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }

                                // Federation Phase 2: forward to federated servers if channel is federated.
                                if state_clone.db.is_channel_federated(&ch).unwrap_or(false) {
                                    let fed_state = state_clone.clone();
                                    let fed_channel = ch.clone();
                                    let fed_content = content.clone();
                                    let fed_display = display.clone();
                                    tokio::spawn(async move {
                                        forward_to_federation(&fed_state, &fed_channel, &fed_display, &fed_content, timestamp).await;
                                    });
                                }

                                // Link previews placeholder (future feature).
                            }
                            // Typing indicator — broadcast to other peers (rate limited).
                            RelayMessage::Typing { .. } => {
                                // Rate limit: max 1 typing indicator per TYPING_RATE_LIMIT_SECS per user.
                                let now = Instant::now();
                                {
                                    let mut typing_ts = state_clone.typing_timestamps.write().await;
                                    if let Some(last) = typing_ts.get(&my_key_for_recv) {
                                        if now.duration_since(*last).as_secs() < TYPING_RATE_LIMIT_SECS {
                                            continue; // Silently drop too-frequent typing indicators
                                        }
                                    }
                                    typing_ts.insert(my_key_for_recv.clone(), now);
                                }
                                let peer = state_clone
                                    .peers
                                    .read()
                                    .await
                                    .get(&my_key_for_recv)
                                    .cloned();
                                let display = peer.as_ref()
                                    .and_then(|p| p.display_name.clone());
                                let typing = RelayMessage::Typing {
                                    from: my_key_for_recv.clone(),
                                    from_name: display,
                                };
                                let _ = state_clone.broadcast_tx.send(typing);
                            }
                            // Reaction — persist and broadcast to all peers.
                            RelayMessage::Reaction { target_from, target_timestamp, emoji, channel: reaction_channel, .. } => {
                                // L-5: Whitelist-only emoji reactions.
                                if !ALLOWED_REACTIONS.contains(&emoji.as_str()) {
                                    continue; // Silently drop reactions not in whitelist
                                }
                                let peer = state_clone.peers.read().await.get(&my_key_for_recv).cloned();
                                let display = peer.as_ref().and_then(|p| p.display_name.clone());
                                let ch = if reaction_channel.is_empty() { "general".to_string() } else { reaction_channel };
                                let _ = state_clone.db.toggle_reaction(
                                    &target_from, target_timestamp, &emoji,
                                    &my_key_for_recv, display.as_deref().unwrap_or(""), &ch,
                                );
                                let reaction = RelayMessage::Reaction {
                                    target_from, target_timestamp, emoji,
                                    from: my_key_for_recv.clone(), from_name: display, channel: ch,
                                };
                                let _ = state_clone.broadcast_tx.send(reaction);
                            }
                            // Delete own message — broadcast removal to all peers.
                            RelayMessage::Delete { timestamp, .. } => {
                                if let Err(e) = state_clone.db.delete_message(&my_key_for_recv, timestamp) {
                                    tracing::error!("Failed to delete message: {e}");
                                }
                                let del = RelayMessage::Delete { from: my_key_for_recv.clone(), timestamp };
                                let _ = state_clone.broadcast_tx.send(del);
                            }
                            // Edit own message — validate and broadcast.
                            RelayMessage::Edit { timestamp, new_content, channel: edit_channel, .. } => {
                                let edit_role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                let edit_char_limit: usize = if edit_role == "admin" { 10_000 } else { 2_000 };
                                if new_content.is_empty() || new_content.len() > edit_char_limit {
                                    let private = RelayMessage::Private {
                                        to: my_key_for_recv.clone(),
                                        message: format!("Edit failed: message must be 1-{} characters.", edit_char_limit),
                                    };
                                    let _ = state_clone.broadcast_tx.send(private);
                                } else {
                                    match state_clone.db.edit_message(&my_key_for_recv, timestamp, &new_content) {
                                        Ok(true) => {
                                            let ch = if edit_channel.is_empty() { "general".to_string() } else { edit_channel };
                                            let edit = RelayMessage::Edit {
                                                from: my_key_for_recv.clone(), timestamp, new_content, channel: ch,
                                            };
                                            let _ = state_clone.broadcast_tx.send(edit);
                                        }
                                        Ok(false) => {
                                            let private = RelayMessage::Private {
                                                to: my_key_for_recv.clone(),
                                                message: "Edit failed: message not found or not yours.".to_string(),
                                            };
                                            let _ = state_clone.broadcast_tx.send(private);
                                        }
                                        Err(e) => tracing::error!("Failed to edit message: {e}"),
                                    }
                                }
                            }
                            // Pin request — pin a specific message by key + timestamp.
                            RelayMessage::PinRequest { from_key, from_name, content, timestamp, channel: pin_ch } => {
                                let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                if role != "admin" && role != "mod" {
                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins and mods can pin messages.".to_string() };
                                    let _ = state_clone.broadcast_tx.send(private);
                                } else {
                                    let ch = if pin_ch.is_empty() { "general".to_string() } else { pin_ch };
                                    let display = state_clone.peers.read().await.get(&my_key_for_recv)
                                        .and_then(|p| p.display_name.clone())
                                        .unwrap_or_else(|| my_key_for_recv[..8].to_string());
                                    match state_clone.db.pin_message(&ch, &from_key, &from_name, &content, timestamp, &display) {
                                        Ok(true) => {
                                            let pin = PinData {
                                                from_key, from_name: from_name.clone(), content: content.clone(),
                                                original_timestamp: timestamp, pinned_by: display.clone(),
                                                pinned_at: std::time::SystemTime::now()
                                                    .duration_since(std::time::UNIX_EPOCH)
                                                    .unwrap_or_default().as_millis() as u64,
                                            };
                                            let _ = state_clone.broadcast_tx.send(RelayMessage::PinAdded { channel: ch.clone(), pin });
                                            let sys = RelayMessage::System { message: format!("📌 {} pinned a message by {}.", display, from_name) };
                                            let _ = state_clone.broadcast_tx.send(sys);
                                        }
                                        Ok(false) => {
                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "That message is already pinned.".to_string() };
                                            let _ = state_clone.broadcast_tx.send(private);
                                        }
                                        Err(e) => {
                                            tracing::error!("Pin request error: {e}");
                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: format!("Pin failed: {e}") };
                                            let _ = state_clone.broadcast_tx.send(private);
                                        }
                                    }
                                }
                            }
                            // Profile update — validate, save, and broadcast.
                            RelayMessage::ProfileUpdate { bio, socials, avatar_url, banner_url, pronouns, location, website, privacy } => {
                                handle_profile_update(&state_clone, &my_key_for_recv, &mut last_profile_update, bio, socials, avatar_url, banner_url, pronouns, location, website, privacy).await;
                            }
                            // Profile request — look up the profile, apply privacy filter, and unicast to requester.
                            RelayMessage::ProfileRequest { name } => {
                                handle_profile_request(&state_clone, &my_key_for_recv, name).await;
                            }
                            // DM — send a direct message.
                            RelayMessage::Dm { to, content, encrypted, nonce, .. } => {
                                handle_dm(&state_clone, &my_key_for_recv, to, content, encrypted, nonce).await;
                            }
                            // DM open — load conversation history.
                            RelayMessage::DmOpen { partner } => {
                                handle_dm_open(&state_clone, &my_key_for_recv, partner).await;
                            }
                            // Voice call signaling — forward to target peer.
                            RelayMessage::VoiceCall { to, action, .. } => {
                                handle_voice_call(&state_clone, &my_key_for_recv, to, action).await;
                            }
                            // WebRTC signaling — forward to target peer.
                            RelayMessage::WebrtcSignal { to, signal_type, data, .. } => {
                                handle_webrtc_signal(&state_clone, &my_key_for_recv, to, signal_type, data).await;
                            }
                            // DM read — mark messages from partner as read.
                            RelayMessage::DmRead { partner } => {
                                handle_dm_read(&state_clone, &my_key_for_recv, partner).await;
                            }
                            // ── Search ──
                            RelayMessage::Search { query, channel, from, limit } => {
                                if query.len() < 2 || query.len() > 200 {
                                    continue;
                                }
                                // Rate limit: 1 search per 2 seconds per user
                                {
                                    let mut last_searches = state_clone.last_search_times.lock().unwrap();
                                    let now = std::time::Instant::now();
                                    if let Some(last) = last_searches.get(&my_key_for_recv) {
                                        if now.duration_since(*last).as_secs() < 2 {
                                            continue;
                                        }
                                    }
                                    last_searches.insert(my_key_for_recv.clone(), now);
                                }
                                let max_results = limit.unwrap_or(50).min(100) as usize;
                                match state_clone.db.search_messages_full(&query, channel.as_deref(), from.as_deref(), max_results, &my_key_for_recv) {
                                    Ok(results) => {
                                        let total = results.len() as u32;
                                        let search_results: Vec<SearchResultData> = results.into_iter().map(|(id, ch, msg)| {
                                            if let RelayMessage::Chat { from, from_name, content, timestamp, .. } = msg {
                                                SearchResultData {
                                                    message_id: id,
                                                    channel: ch,
                                                    from: from.clone(),
                                                    from_name: from_name.unwrap_or_else(|| shortKey_rust(&from)),
                                                    content,
                                                    timestamp,
                                                }
                                            } else {
                                                SearchResultData {
                                                    message_id: id, channel: ch, from: String::new(),
                                                    from_name: String::new(), content: String::new(), timestamp: 0,
                                                }
                                            }
                                        }).collect();
                                        let _ = state_clone.broadcast_tx.send(RelayMessage::SearchResults {
                                            target: Some(my_key_for_recv.clone()),
                                            query,
                                            results: search_results,
                                            total,
                                        });
                                    }
                                    Err(e) => {
                                        tracing::error!("Search error: {e}");
                                    }
                                }
                            }
                            // ── Thread request ──
                            RelayMessage::ThreadRequest { from, timestamp } => {
                                match state_clone.db.get_thread(&from, timestamp, 100) {
                                    Ok(replies) => {
                                        let messages: Vec<ThreadMessageData> = replies.into_iter().map(|(fk, fn_, c, ts, ch)| {
                                            ThreadMessageData { from: fk, from_name: fn_, content: c, timestamp: ts, channel: ch }
                                        }).collect();
                                        let _ = state_clone.broadcast_tx.send(RelayMessage::ThreadResponse {
                                            target: Some(my_key_for_recv.clone()),
                                            parent_from: from,
                                            parent_timestamp: timestamp,
                                            messages,
                                        });
                                    }
                                    Err(e) => {
                                        tracing::error!("Thread request error: {e}");
                                    }
                                }
                            }
                            // ── Delete by ID ──
                            RelayMessage::DeleteById { message_id } => {
                                let user_role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                let is_admin = user_role == "admin" || user_role == "mod";
                                match state_clone.db.delete_message_by_id(message_id, &my_key_for_recv, is_admin) {
                                    Ok(Some((from_key, channel_id))) => {
                                        // Find the timestamp from the message (we need it for client-side removal).
                                        // Broadcast deletion to all clients.
                                        let _ = state_clone.broadcast_tx.send(RelayMessage::MessageDeleted {
                                            message_id,
                                            channel: channel_id,
                                            from: from_key,
                                            timestamp: 0, // Client uses message_id for removal
                                        });
                                    }
                                    Ok(None) => {
                                        let private = RelayMessage::Private {
                                            to: my_key_for_recv.clone(),
                                            message: "Cannot delete: message not found or not yours.".to_string(),
                                        };
                                        let _ = state_clone.broadcast_tx.send(private);
                                    }
                                    Err(e) => {
                                        tracing::error!("Delete by ID error: {e}");
                                    }
                                }
                            }
                            // ── Set Status ──
                            RelayMessage::SetStatus { status, text } => {
                                let valid_statuses = ["online", "away", "busy", "dnd"];
                                if !valid_statuses.contains(&status.as_str()) {
                                    continue;
                                }
                                let status_text = if text.len() > 128 { text[..128].to_string() } else { text };
                                let peer = state_clone.peers.read().await.get(&my_key_for_recv).cloned();
                                if let Some(ref p) = peer {
                                    if let Some(ref name) = p.display_name {
                                        let _ = state_clone.db.save_user_status(name, &status, &status_text);
                                        state_clone.user_statuses.write().await.insert(name.to_lowercase(), (status.clone(), status_text.clone()));
                                        // Broadcast updated peer/user lists so everyone sees the status change.
                                        broadcast_peer_list(&state_clone).await;
                                        broadcast_full_user_list(&state_clone).await;
                                    }
                                }
                            }
                            // ── Voice Rooms (Persistent Channels) ──
                            // ── Voice Rooms (Persistent Channels) ──
                            RelayMessage::VoiceRoom { action, room_id, room_name } => {
                                handle_voice_room(&state_clone, &my_key_for_recv, action, room_id, room_name).await;
                            }
                            // ── Voice Room WebRTC Signaling ──
                            RelayMessage::VoiceRoomSignal { to, room_id, signal_type, data, .. } => {
                                handle_voice_room_signal(&state_clone, &my_key_for_recv, to, room_id, signal_type, data).await;
                            }
                            // ── Project Board: Task List ──
                            RelayMessage::TaskList { } => {
                                handle_task_list(&state_clone, &my_key_for_recv).await;
                            }
                            // ── Project Board: Create Task ──
                            RelayMessage::TaskCreate { title, description, status, priority, assignee, labels, project } => {
                                handle_task_create(&state_clone, &my_key_for_recv, title, description, status, priority, assignee, labels, project).await;
                            }
                            // ── Project Board: Update Task ──
                            RelayMessage::TaskUpdate { id, title, description, priority, assignee, labels, project } => {
                                handle_task_update_msg(&state_clone, &my_key_for_recv, id, title, description, priority, assignee, labels, project).await;
                            }
                            // ── Project Board: Move Task ──
                            RelayMessage::TaskMove { id, status } => {
                                handle_task_move(&state_clone, &my_key_for_recv, id, status).await;
                            }
                            // ── Project Board: Delete Task ──
                            RelayMessage::TaskDelete { id } => {
                                handle_task_delete(&state_clone, &my_key_for_recv, id).await;
                            }
                            // ── Project Board: Add Comment ──
                            RelayMessage::TaskComment { task_id, content } => {
                                handle_task_comment_msg(&state_clone, &my_key_for_recv, task_id, content).await;
                            }
                            // ── Project Board: Request Comments ──
                            RelayMessage::TaskCommentsRequest { task_id } => {
                                handle_task_comments_request(&state_clone, &my_key_for_recv, task_id).await;
                            }
                            // ── Projects ──
                            RelayMessage::ProjectList {} => {
                                handle_project_list(&state_clone, &my_key_for_recv).await;
                            }
                            RelayMessage::ProjectCreate { name, description, visibility, color, icon } => {
                                handle_project_create(&state_clone, &my_key_for_recv, name, description, visibility, color, icon).await;
                            }
                            RelayMessage::ProjectUpdate { id, name, description, visibility, color, icon } => {
                                handle_project_update(&state_clone, &my_key_for_recv, id, name, description, visibility, color, icon).await;
                            }
                            RelayMessage::ProjectDelete { id } => {
                                handle_project_delete(&state_clone, &my_key_for_recv, id).await;
                            }
                            // ── Follow/Unfollow ──
                            RelayMessage::Follow { target_key } => {
                                handle_follow(&state_clone, &my_key_for_recv, target_key).await;
                            }
                            RelayMessage::Unfollow { target_key } => {
                                handle_unfollow(&state_clone, &my_key_for_recv, target_key).await;
                            }
                            // ── Friend Codes ──
                            RelayMessage::FriendCodeRequest {} => {
                                handle_friend_code_request(&state_clone, &my_key_for_recv).await;
                            }
                            RelayMessage::FriendCodeRedeem { code } => {
                                handle_friend_code_redeem(&state_clone, &my_key_for_recv, code).await;
                            }
                            // ── Server Membership ──
                            RelayMessage::MemberListRequest { limit, offset, search } => {
                                handle_member_list_request(&state_clone, &my_key_for_recv, limit, offset, search).await;
                            }
                            // ── Marketplace ──
                            RelayMessage::ListingBrowse {} => {
                                handle_listing_browse(&state_clone, &my_key_for_recv).await;
                            }
                            RelayMessage::ListingCreate { id, title, description, category, condition, price, payment_methods, location } => {
                                handle_listing_create(&state_clone, &my_key_for_recv, id, title, description, category, condition, price, payment_methods, location).await;
                            }
                            RelayMessage::ListingUpdate { id, title, description, category, condition, price, payment_methods, location, status } => {
                                handle_listing_update(&state_clone, &my_key_for_recv, id, title, description, category, condition, price, payment_methods, location, status).await;
                            }
                            RelayMessage::ListingDelete { id } => {
                                handle_listing_delete(&state_clone, &my_key_for_recv, id).await;
                            }
                            // ── Reviews ──
                            RelayMessage::ReviewCreate { listing_id, rating, comment } => {
                                handle_review_create(&state_clone, &my_key_for_recv, listing_id, rating, comment).await;
                            }
                            RelayMessage::ReviewDelete { listing_id, review_id } => {
                                handle_review_delete(&state_clone, &my_key_for_recv, listing_id, review_id).await;
                            }
                            // ── Listing Messages (buyer-seller) ──
                            RelayMessage::ListingMessageSend { listing_id, content } => {
                                handle_listing_message_send(&state_clone, &my_key_for_recv, listing_id, content).await;
                            }
                            RelayMessage::ListingMessageHistory { listing_id } => {
                                handle_listing_message_history(&state_clone, &my_key_for_recv, listing_id).await;
                            }
                            // ── Notification Preferences ──
                            RelayMessage::UpdateNotificationPrefs { dm, mentions, tasks, dnd_start, dnd_end } => {
                                handle_update_notification_prefs(&state_clone, &my_key_for_recv, dm, mentions, tasks, dnd_start, dnd_end).await;
                            }
                            RelayMessage::GetNotificationPrefs {} => {
                                handle_get_notification_prefs(&state_clone, &my_key_for_recv).await;
                            }
                            // ── Group System ──
                            RelayMessage::GroupCreate { name } => {
                                handle_group_create(&state_clone, &my_key_for_recv, name).await;
                            }
                            RelayMessage::GroupJoin { invite_code } => {
                                handle_group_join(&state_clone, &my_key_for_recv, invite_code).await;
                            }
                            RelayMessage::GroupLeave { group_id } => {
                                handle_group_leave(&state_clone, &my_key_for_recv, group_id).await;
                            }
                            RelayMessage::GroupHistoryRequest { group_id } => {
                                handle_group_history_request(&state_clone, &my_key_for_recv, group_id).await;
                            }
                            RelayMessage::GroupMembersRequest { group_id } => {
                                handle_group_members_request(&state_clone, &my_key_for_recv, group_id).await;
                            }
                            RelayMessage::GroupMsg { group_id, content } => {
                                handle_group_msg(&state_clone, &my_key_for_recv, group_id, content).await;
                            }
                            // ── Device management ──
                            RelayMessage::DeviceListRequest {} => {
                                handle_device_list_request(&state_clone, &my_key_for_recv).await;
                            }
                            RelayMessage::DeviceLabel { public_key, label } => {
                                handle_device_label(&state_clone, &my_key_for_recv, public_key, label).await;
                            }
                            RelayMessage::DeviceRevoke { key_prefix } => {
                                handle_device_revoke(&state_clone, &my_key_for_recv, key_prefix).await;
                            }
                            // ── Federation Phase 2: incoming federation messages ──
                            RelayMessage::FederationHello { server_id, public_key, name, version, timestamp, signature } => {
                                handle_federation_hello(&state_clone, &my_key_for_recv, server_id, public_key, name, version, timestamp, signature).await;
                            }
                            RelayMessage::FederatedChat { server_id, server_name, from_name, content, timestamp, channel, signature } => {
                                handle_federated_chat(&state_clone, server_id, server_name, from_name, content, timestamp, channel, signature).await;
                            }
                            RelayMessage::FederationWelcome { server_id, name, channels } => {
                                handle_federation_welcome(&state_clone, server_id, name, channels).await;
                            }
                            // Profile gossip from a federated server connecting to us.
                            RelayMessage::ProfileGossip { public_key, name, bio, avatar_url, banner_url, socials, pronouns, location, website, timestamp, signature } => {
                                tracing::debug!("Profile gossip received for {} via direct WS", &name);
                                let _ = state_clone.db.store_signed_profile(
                                    &public_key, &name, &bio, &avatar_url, &banner_url, &socials, &pronouns, &location, &website, timestamp, &signature,
                                );
                            }
                            // ── Streaming ──
                            RelayMessage::StreamStart { title, category } => {
                                handle_stream_start(&state_clone, &my_key_for_recv, title, category).await;
                            }
                            RelayMessage::StreamStop {} => {
                                handle_stream_stop(&state_clone, &my_key_for_recv).await;
                            }
                            RelayMessage::StreamOffer { to, data, .. } => {
                                handle_stream_offer(&state_clone, &my_key_for_recv, to, data).await;
                            }
                            RelayMessage::StreamAnswer { to, data, .. } => {
                                handle_stream_answer(&state_clone, &my_key_for_recv, to, data).await;
                            }
                            RelayMessage::StreamIce { to, data, .. } => {
                                handle_stream_ice(&state_clone, &my_key_for_recv, to, data).await;
                            }
                            RelayMessage::StreamViewerJoin { .. } => {
                                handle_stream_viewer_join(&state_clone, &my_key_for_recv).await;
                            }
                            RelayMessage::StreamViewerLeave { .. } => {
                                handle_stream_viewer_leave(&state_clone, &my_key_for_recv).await;
                            }
                            RelayMessage::StreamChat { content, source, source_user, .. } => {
                                handle_stream_chat(&state_clone, &my_key_for_recv, content, source, source_user).await;
                            }
                            RelayMessage::StreamInfoRequest {} => {
                                handle_stream_info_request(&state_clone).await;
                            }
                            RelayMessage::StreamSetExternal { urls } => {
                                handle_stream_set_external(&state_clone, &my_key_for_recv, urls).await;
                            }

                            _ => {
                                // Ignore other message types from clients.
                            }
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for either task to finish.
    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }

    // Clean up: remove peer, clear kicked status, remove upload token, and announce departure.
    let disconnected_role = state.db.get_role(&my_key).unwrap_or_default();
    {
        let mut peers = state.peers.write().await;
        if let Some(peer) = peers.remove(&my_key) {
            if let Some(ref token) = peer.upload_token {
                state.upload_tokens.write().await.remove(token);
            }
        }
    }
    state.kicked_keys.write().await.remove(&my_key);

    // Remove player from game world on disconnect.
    handle_game_disconnect(&state, &my_key).await;

    // Remove from voice rooms and clear status text on disconnect.
    leave_voice_room(&state, &my_key).await;
    // Clear status text on disconnect (keep status preference).
    if let Ok(Some(name)) = state.db.name_for_key(&my_key) {
        let _ = state.db.clear_user_status_text(&name);
        if let Some(entry) = state.user_statuses.write().await.get_mut(&name.to_lowercase()) {
            entry.1 = String::new(); // clear status text
        }
    }

    info!("Peer disconnected: {my_key}");
    let _ = state.broadcast_tx.send(RelayMessage::PeerLeft {
        public_key: my_key,
    });

    // Broadcast updated full user list to all clients.
    broadcast_full_user_list(&state).await;

    // Auto-lockdown with grace period: if no admins/mods remain, wait 30s
    // before locking down. Prevents false lockdowns during deploy restarts.
    // Disabled when REGISTRATION_DEFAULT_OPEN=true (default behavior).
    if (disconnected_role == "admin" || disconnected_role == "mod") && !registration_default_open() {
        let peers = state.peers.read().await;
        let has_staff = peers.values().any(|p| {
            let role = state.db.get_role(&p.public_key_hex).unwrap_or_default();
            role == "admin" || role == "mod"
        });
        drop(peers);

        if !has_staff {
            let state_for_lockdown = state.clone();
            tokio::spawn(async move {
                // Grace period: wait 30 seconds before locking down.
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;

                // Re-check: did a staff member reconnect during the grace period?
                let peers = state_for_lockdown.peers.read().await;
                let has_staff_now = peers.values().any(|p| {
                    let role = state_for_lockdown.db.get_role(&p.public_key_hex).unwrap_or_default();
                    role == "admin" || role == "mod"
                });
                drop(peers);

                if !has_staff_now {
                    let already_locked = *state_for_lockdown.lockdown.read().await;
                    if !already_locked {
                        *state_for_lockdown.lockdown.write().await = true;
                        *state_for_lockdown.auto_lockdown.write().await = true;
                        // L-4: Persist lockdown state.
                        let _ = state_for_lockdown.db.set_state("lockdown", "true");
                        let sys = RelayMessage::System {
                            message: "🔒 Auto-lockdown: no moderators online (30s grace period expired).".to_string(),
                        };
                        let _ = state_for_lockdown.broadcast_tx.send(sys);
                    }
                }
            });
        }
    }
}

