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
const FIB_DELAYS: [u64; 8] = [1, 1, 2, 3, 5, 8, 13, 21];

/// Duration after which a new identity is no longer considered "new" (10 minutes).
const NEW_ACCOUNT_WINDOW_SECS: u64 = 600;

/// Flat rate limit for new accounts (seconds).
const NEW_ACCOUNT_DELAY_SECS: u64 = 5;

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
    http_client: reqwest::Client,
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
        if persisted_lockdown {
            info!("Restored lockdown state from database: locked");
        }

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
            lockdown: RwLock::new(persisted_lockdown),
            auto_lockdown: RwLock::new(false),
            kicked_keys: RwLock::new(HashSet::new()),
            connection_count: AtomicUsize::new(0),
            typing_timestamps: RwLock::new(HashMap::new()),
            upload_tokens: RwLock::new(HashMap::new()),
        }
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
    },

    /// Server announces a peer joined.
    #[serde(rename = "peer_joined")]
    PeerJoined {
        public_key: String,
        display_name: Option<String>,
        #[serde(default)]
        role: String,
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

    /// Client sends a profile update (bio + socials).
    #[serde(rename = "profile_update")]
    ProfileUpdate {
        bio: String,
        socials: String,
    },

    /// Server sends profile data for a specific user.
    #[serde(rename = "profile_data")]
    ProfileData {
        name: String,
        bio: String,
        socials: String,
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
}

/// DM data sent to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DmData {
    pub from: String,
    pub from_name: String,
    pub to: String,
    pub content: String,
    pub timestamp: u64,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub public_key: String,
    pub display_name: Option<String>,
    #[serde(default)]
    pub role: String,
    /// Per-session upload token (only set for the recipient's own entry).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upload_token: Option<String>,
}

/// Info about a registered user (online or offline) for the full user list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub name: String,
    pub public_key: String,
    pub role: String,
    pub online: bool,
    pub key_count: usize,
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
            if let Ok(RelayMessage::Identify { public_key, display_name, link_code, invite_code, bot_secret }) =
                serde_json::from_str::<RelayMessage>(&text)
            {
                // L-1: Bot keys require bot_secret matching API_SECRET.
                if public_key.starts_with("bot_") {
                    let expected = std::env::var("API_SECRET").unwrap_or_default();
                    let provided = bot_secret.as_deref().unwrap_or("");
                    if expected.is_empty() || provided != expected {
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

                let peer = Peer {
                    public_key_hex: public_key.clone(),
                    display_name: final_name.clone(),
                    upload_token: Some(upload_token.clone()),
                };

                // Register peer and upload token mapping.
                state.peers.write().await.insert(public_key.clone(), peer);
                state.upload_tokens.write().await.insert(upload_token.clone(), public_key.clone());
                peer_key = Some(public_key.clone());

                info!("Peer connected: {public_key} ({:?})", final_name);

                // Send current peer list to the new peer (with their upload_token).
                let peers: Vec<PeerInfo> = state
                    .peers
                    .read()
                    .await
                    .values()
                    .map(|p| {
                        let role = state.db.get_role(&p.public_key_hex).unwrap_or_default();
                        let token = if p.public_key_hex == public_key {
                            p.upload_token.clone()
                        } else {
                            None
                        };
                        PeerInfo {
                            public_key: p.public_key_hex.clone(),
                            display_name: p.display_name.clone(),
                            role,
                            upload_token: token,
                        }
                    })
                    .collect();

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
                    let users: Vec<UserInfo> = all_users
                        .into_iter()
                        .map(|(name, first_key, role, key_count)| {
                            let online = online_names.contains(&name.to_lowercase());
                            UserInfo { name, public_key: first_key, role, online, key_count }
                        })
                        .collect();
                    let ful_msg = serde_json::to_string(&RelayMessage::FullUserList { users }).unwrap();
                    let _ = ws_tx.send(Message::Text(ful_msg.into())).await;
                }

                // Send the user's own profile data if it exists.
                if let Some(ref name) = final_name {
                    if let Ok(Some((bio, socials))) = state.db.get_profile(name) {
                        let profile_msg = serde_json::to_string(&RelayMessage::ProfileData {
                            name: name.clone(),
                            bio,
                            socials,
                        }).unwrap();
                        let _ = ws_tx.send(Message::Text(profile_msg.into())).await;
                    }
                }

                // Send channel list.
                if let Ok(channels) = state.db.list_channels() {
                    let channel_infos: Vec<ChannelInfo> = channels.into_iter().map(|(id, name, desc, ro)| {
                        ChannelInfo { id, name, description: desc, read_only: ro }
                    }).collect();
                    let ch_msg = serde_json::to_string(&RelayMessage::ChannelList { channels: channel_infos }).unwrap();
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

                // Announce to everyone.
                let peer_role = state.db.get_role(&public_key).unwrap_or_default();
                let _ = state.broadcast_tx.send(RelayMessage::PeerJoined {
                    public_key,
                    display_name: final_name,
                    role: peer_role.clone(),
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

            // Don't echo chat/typing/delete/edit messages back to the sender.
            let should_skip = match &msg {
                RelayMessage::Chat { from, .. } => from == &my_key_for_broadcast,
                RelayMessage::Typing { from, .. } => from == &my_key_for_broadcast,
                RelayMessage::Delete { from, .. } => from == &my_key_for_broadcast,
                RelayMessage::Reaction { from, .. } => from == &my_key_for_broadcast,
                RelayMessage::Edit { from, .. } => from == &my_key_for_broadcast,
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
                    if let Ok(relay_msg) = serde_json::from_str::<RelayMessage>(&text) {
                        match relay_msg {
                            RelayMessage::Chat { content, timestamp, signature, channel, .. } => {
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
                                        RateLimitState {
                                            first_seen: now,
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
                                                            if let Ok(channels) = state_clone.db.list_channels() {
                                                                let infos: Vec<ChannelInfo> = channels.into_iter().map(|(id, name, desc, ro)| ChannelInfo { id, name, description: desc, read_only: ro }).collect();
                                                                let _ = state_clone.broadcast_tx.send(RelayMessage::ChannelList { channels: infos });
                                                            }
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
                                        "/channel-delete" => {
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins can delete channels.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                let ch_name = trimmed.split_whitespace().nth(1).unwrap_or("");
                                                if ch_name == "general" {
                                                    let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Cannot delete the general channel.".to_string() };
                                                    let _ = state_clone.broadcast_tx.send(private);
                                                } else if state_clone.db.delete_channel(ch_name).unwrap_or(false) {
                                                    if let Ok(channels) = state_clone.db.list_channels() {
                                                        let infos: Vec<ChannelInfo> = channels.into_iter().map(|(id, name, desc, ro)| ChannelInfo { id, name, description: desc, read_only: ro }).collect();
                                                        let _ = state_clone.broadcast_tx.send(RelayMessage::ChannelList { channels: infos });
                                                    }
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
                                                            if let Ok(channels) = state_clone.db.list_channels() {
                                                                let infos: Vec<ChannelInfo> = channels.into_iter().map(|(id, name, desc, ro)| ChannelInfo { id, name, description: desc, read_only: ro }).collect();
                                                                let _ = state_clone.broadcast_tx.send(RelayMessage::ChannelList { channels: infos });
                                                            }
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
                                                            if let Ok(channels) = state_clone.db.list_channels() {
                                                                let infos: Vec<ChannelInfo> = channels.into_iter().map(|(id, name, desc, ro)| ChannelInfo { id, name, description: desc, read_only: ro }).collect();
                                                                let _ = state_clone.broadcast_tx.send(RelayMessage::ChannelList { channels: infos });
                                                            }
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
                                            } else if dm_content.len() > 2000 {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "DM too long (max 2000 chars).".to_string() };
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
                                                        // Send to recipient.
                                                        let dm_msg = RelayMessage::Dm {
                                                            from: my_key_for_recv.clone(),
                                                            from_name: Some(display.clone()),
                                                            to: target_key.clone(),
                                                            content: dm_content.clone(),
                                                            timestamp: ts,
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
                                            } else if new_content.len() > 2000 {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Message too long (max 2000 chars).".to_string() };
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

                                let chat = RelayMessage::Chat {
                                    from: my_key_for_recv.clone(),
                                    from_name: Some(display.clone()),
                                    content: content.clone(),
                                    timestamp,
                                    signature: broadcast_sig,
                                    channel: ch.clone(),
                                };

                                // Store in channel-specific table.
                                if let Err(e) = state_clone.db.store_message_in_channel(&chat, &ch) {
                                    tracing::error!("Failed to persist message: {e}");
                                }
                                // Broadcast to all (clients filter by their active channel).
                                let _ = state_clone.broadcast_tx.send(chat);

                                // Notify webhook for human messages (non-bot keys).
                                if !my_key_for_recv.starts_with("bot_") {
                                    state_clone.notify_webhook(&display, &content);
                                }
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
                                // Persist the reaction toggle.
                                let _ = state_clone.db.toggle_reaction(
                                    &target_from,
                                    target_timestamp,
                                    &emoji,
                                    &my_key_for_recv,
                                    display.as_deref().unwrap_or(""),
                                    &ch,
                                );
                                let reaction = RelayMessage::Reaction {
                                    target_from,
                                    target_timestamp,
                                    emoji,
                                    from: my_key_for_recv.clone(),
                                    from_name: display,
                                    channel: ch,
                                };
                                let _ = state_clone.broadcast_tx.send(reaction);
                            }
                            // Delete own message — broadcast removal to all peers.
                            RelayMessage::Delete { timestamp, .. } => {
                                // Only allow deleting your own messages.
                                if let Err(e) = state_clone.db.delete_message(&my_key_for_recv, timestamp) {
                                    tracing::error!("Failed to delete message: {e}");
                                }
                                let del = RelayMessage::Delete {
                                    from: my_key_for_recv.clone(),
                                    timestamp,
                                };
                                let _ = state_clone.broadcast_tx.send(del);
                            }
                            // Edit own message — validate and broadcast.
                            RelayMessage::Edit { timestamp, new_content, channel: edit_channel, .. } => {
                                // Validate: content not empty, <= 2000 chars.
                                if new_content.is_empty() || new_content.len() > 2000 {
                                    let private = RelayMessage::Private {
                                        to: my_key_for_recv.clone(),
                                        message: "Edit failed: message must be 1-2000 characters.".to_string(),
                                    };
                                    let _ = state_clone.broadcast_tx.send(private);
                                } else {
                                    match state_clone.db.edit_message(&my_key_for_recv, timestamp, &new_content) {
                                        Ok(true) => {
                                            let ch = if edit_channel.is_empty() { "general".to_string() } else { edit_channel };
                                            let edit = RelayMessage::Edit {
                                                from: my_key_for_recv.clone(),
                                                timestamp,
                                                new_content,
                                                channel: ch,
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
                                        Err(e) => {
                                            tracing::error!("Failed to edit message: {e}");
                                        }
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
                                                from_key,
                                                from_name: from_name.clone(),
                                                content: content.clone(),
                                                original_timestamp: timestamp,
                                                pinned_by: display.clone(),
                                                pinned_at: std::time::SystemTime::now()
                                                    .duration_since(std::time::UNIX_EPOCH)
                                                    .unwrap_or_default()
                                                    .as_millis() as u64,
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
                            // Profile update — save and broadcast.
                            RelayMessage::ProfileUpdate { bio, socials } => {
                                let peer = state_clone.peers.read().await.get(&my_key_for_recv).cloned();
                                let display = peer.as_ref().and_then(|p| p.display_name.clone());
                                if let Some(ref name) = display {
                                    // Rate limit: max 1 profile update per 30 seconds.
                                    let now = Instant::now();
                                    if let Some(last) = last_profile_update {
                                        if now.duration_since(last).as_secs() < 30 {
                                            let private = RelayMessage::Private {
                                                to: my_key_for_recv.clone(),
                                                message: "⏳ Profile update rate limited. Please wait 30 seconds between updates.".to_string(),
                                            };
                                            let _ = state_clone.broadcast_tx.send(private);
                                            continue;
                                        }
                                    }

                                    // Sanitize: strip HTML tags from bio.
                                    let clean_bio: String = {
                                        let mut result = String::new();
                                        let mut in_tag = false;
                                        for ch in bio.chars() {
                                            match ch {
                                                '<' => in_tag = true,
                                                '>' => { in_tag = false; }
                                                _ if !in_tag => result.push(ch),
                                                _ => {}
                                            }
                                        }
                                        result
                                    };

                                    // Validate bio length.
                                    if clean_bio.len() > 280 {
                                        let private = RelayMessage::Private {
                                            to: my_key_for_recv.clone(),
                                            message: "Bio too long (max 280 characters).".to_string(),
                                        };
                                        let _ = state_clone.broadcast_tx.send(private);
                                        continue;
                                    }

                                    // Validate socials JSON.
                                    if socials.len() > 1024 || serde_json::from_str::<serde_json::Value>(&socials).is_err() {
                                        let private = RelayMessage::Private {
                                            to: my_key_for_recv.clone(),
                                            message: "Invalid socials data.".to_string(),
                                        };
                                        let _ = state_clone.broadcast_tx.send(private);
                                        continue;
                                    }

                                    // Validate URLs in socials: reject dangerous URIs,
                                    // require https:// for URL fields.
                                    if let Ok(socials_obj) = serde_json::from_str::<serde_json::Value>(&socials) {
                                        if let Some(obj) = socials_obj.as_object() {
                                            let url_fields = ["website", "youtube"];
                                            let handle_fields = ["twitter", "github"];
                                            let mut invalid = false;

                                            for field in &url_fields {
                                                if let Some(serde_json::Value::String(val)) = obj.get(*field) {
                                                    let val = val.trim();
                                                    if val.is_empty() { continue; }
                                                    let lower = val.to_lowercase();
                                                    if lower.starts_with("javascript:") || lower.starts_with("data:") {
                                                        invalid = true;
                                                        break;
                                                    }
                                                    if !val.starts_with("https://") {
                                                        let private = RelayMessage::Private {
                                                            to: my_key_for_recv.clone(),
                                                            message: format!("Profile URL for '{}' must start with https://", field),
                                                        };
                                                        let _ = state_clone.broadcast_tx.send(private);
                                                        invalid = true;
                                                        break;
                                                    }
                                                }
                                            }

                                            if !invalid {
                                                for field in &handle_fields {
                                                    if let Some(serde_json::Value::String(val)) = obj.get(*field) {
                                                        let val = val.trim();
                                                        if val.is_empty() { continue; }
                                                        // Handles: only alphanumeric + underscores.
                                                        if !val.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                                                            let private = RelayMessage::Private {
                                                                to: my_key_for_recv.clone(),
                                                                message: format!("Profile handle for '{}' can only contain letters, numbers, and underscores.", field),
                                                            };
                                                            let _ = state_clone.broadcast_tx.send(private);
                                                            invalid = true;
                                                            break;
                                                        }
                                                    }
                                                }
                                            }

                                            // Also check any other string fields for dangerous URIs.
                                            if !invalid {
                                                for (_key, val) in obj {
                                                    if let Some(s) = val.as_str() {
                                                        let lower = s.trim().to_lowercase();
                                                        if lower.starts_with("javascript:") || lower.starts_with("data:") {
                                                            invalid = true;
                                                            break;
                                                        }
                                                    }
                                                }
                                            }

                                            if invalid {
                                                continue;
                                            }
                                        }
                                    }

                                    // Save to DB.
                                    match state_clone.db.save_profile(name, &clean_bio, &socials) {
                                        Ok(()) => {
                                            last_profile_update = Some(now);
                                            // Broadcast profile data to all peers.
                                            let _ = state_clone.broadcast_tx.send(RelayMessage::ProfileData {
                                                name: name.clone(),
                                                bio: clean_bio,
                                                socials,
                                            });
                                        }
                                        Err(e) => {
                                            tracing::error!("Failed to save profile: {e}");
                                            let private = RelayMessage::Private {
                                                to: my_key_for_recv.clone(),
                                                message: "Failed to save profile.".to_string(),
                                            };
                                            let _ = state_clone.broadcast_tx.send(private);
                                        }
                                    }
                                } else {
                                    let private = RelayMessage::Private {
                                        to: my_key_for_recv.clone(),
                                        message: "You must have a registered name to set a profile.".to_string(),
                                    };
                                    let _ = state_clone.broadcast_tx.send(private);
                                }
                            }
                            // Profile request — look up and send back privately.
                            RelayMessage::ProfileRequest { name } => {
                                match state_clone.db.get_profile(&name) {
                                    Ok(Some((bio, socials))) => {
                                        let pd = RelayMessage::ProfileData {
                                            name: name.clone(),
                                            bio,
                                            socials,
                                        };
                                        // Profile data is public — broadcast to all. The requesting
                                        // client will pick it up and display it.
                                        let _ = state_clone.broadcast_tx.send(pd);
                                    }
                                    Ok(None) => {
                                        // Send empty profile.
                                        let _ = state_clone.broadcast_tx.send(RelayMessage::ProfileData {
                                            name: name.clone(),
                                            bio: String::new(),
                                            socials: "{}".to_string(),
                                        });
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to get profile: {e}");
                                    }
                                }
                            }
                            // DM — send a direct message.
                            RelayMessage::Dm { to, content, .. } => {
                                let peer = state_clone.peers.read().await.get(&my_key_for_recv).cloned();
                                let sender_name = peer.as_ref()
                                    .and_then(|p| p.display_name.clone())
                                    .unwrap_or_else(|| "Anonymous".to_string());

                                // Validate.
                                if content.is_empty() {
                                    continue;
                                }
                                if content.len() > 2000 {
                                    let private = RelayMessage::Private {
                                        to: my_key_for_recv.clone(),
                                        message: "DM too long (max 2000 chars).".to_string(),
                                    };
                                    let _ = state_clone.broadcast_tx.send(private);
                                    continue;
                                }
                                if to == my_key_for_recv {
                                    let private = RelayMessage::Private {
                                        to: my_key_for_recv.clone(),
                                        message: "You can't DM yourself.".to_string(),
                                    };
                                    let _ = state_clone.broadcast_tx.send(private);
                                    continue;
                                }
                                // Rate limiting (same Fibonacci backoff as chat).
                                let user_role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                if user_role == "muted" {
                                    let private = RelayMessage::Private {
                                        to: my_key_for_recv.clone(),
                                        message: "You are muted and cannot send DMs.".to_string(),
                                    };
                                    let _ = state_clone.broadcast_tx.send(private);
                                    continue;
                                }

                                // Fibonacci rate limiting for DMs (skip for bots and admins).
                                if !my_key_for_recv.starts_with("bot_") && user_role != "admin" {
                                    let now = Instant::now();
                                    let mut rate_limits = state_clone.rate_limits.write().await;
                                    let rl = rate_limits.entry(my_key_for_recv.clone()).or_insert_with(|| {
                                        RateLimitState {
                                            first_seen: now,
                                            last_message_time: now - std::time::Duration::from_secs(60),
                                            fib_index: 0,
                                        }
                                    });

                                    let elapsed = now.duration_since(rl.last_message_time).as_secs();
                                    let fib_delay = FIB_DELAYS[rl.fib_index];

                                    let is_trusted = user_role == "verified" || user_role == "donor" || user_role == "mod" || user_role == "admin";
                                    let account_age = now.duration_since(rl.first_seen).as_secs();
                                    let new_account_delay = if !is_trusted && account_age < NEW_ACCOUNT_WINDOW_SECS {
                                        NEW_ACCOUNT_DELAY_SECS
                                    } else {
                                        0
                                    };

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

                                    if elapsed > required_delay {
                                        rl.fib_index = 0;
                                    } else {
                                        rl.fib_index = (rl.fib_index + 1).min(FIB_DELAYS.len() - 1);
                                    }

                                    rl.last_message_time = now;
                                }

                                let ts = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis() as u64;

                                // Store the DM.
                                if let Err(e) = state_clone.db.store_dm(&my_key_for_recv, &sender_name, &to, &content, ts) {
                                    tracing::error!("Failed to store DM: {e}");
                                }

                                // Send to recipient via broadcast (filtered in send loop).
                                let dm_msg = RelayMessage::Dm {
                                    from: my_key_for_recv.clone(),
                                    from_name: Some(sender_name.clone()),
                                    to: to.clone(),
                                    content: content.clone(),
                                    timestamp: ts,
                                };
                                let _ = state_clone.broadcast_tx.send(dm_msg);

                                // Update DM lists for both parties.
                                send_dm_list_update(&state_clone, &my_key_for_recv);
                                send_dm_list_update(&state_clone, &to);
                            }
                            // DM open — load conversation history.
                            RelayMessage::DmOpen { partner } => {
                                // Resolve both parties by name for multi-key support.
                                let my_name = state_clone.db.name_for_key(&my_key_for_recv).ok().flatten();
                                let partner_name = state_clone.db.name_for_key(&partner).ok().flatten();

                                // Mark messages from partner as read (by name if possible).
                                if let (Some(pn), Some(mn)) = (&partner_name, &my_name) {
                                    let _ = state_clone.db.mark_dms_read_by_name(pn, mn);
                                } else {
                                    let _ = state_clone.db.mark_dms_read(&partner, &my_key_for_recv);
                                }

                                // Load conversation by name if possible (merges all keys for each user).
                                let records = if let (Some(mn), Some(pn)) = (&my_name, &partner_name) {
                                    state_clone.db.load_dm_conversation_by_name(mn, pn, 100)
                                } else {
                                    state_clone.db.load_dm_conversation(&my_key_for_recv, &partner, 100)
                                };

                                match records {
                                    Ok(records) => {
                                        let messages: Vec<DmData> = records.into_iter().map(|r| DmData {
                                            from: r.from_key,
                                            from_name: r.from_name,
                                            to: r.to_key,
                                            content: r.content,
                                            timestamp: r.timestamp,
                                        }).collect();
                                        let history = RelayMessage::DmHistory {
                                            target: Some(my_key_for_recv.clone()),
                                            partner,
                                            messages,
                                        };
                                        let _ = state_clone.broadcast_tx.send(history);
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to load DM history: {e}");
                                    }
                                }
                                // Also update DM list (to clear unread count).
                                send_dm_list_update(&state_clone, &my_key_for_recv);
                            }
                            // DM read — mark messages from partner as read.
                            RelayMessage::DmRead { partner } => {
                                let my_name = state_clone.db.name_for_key(&my_key_for_recv).ok().flatten();
                                let partner_name = state_clone.db.name_for_key(&partner).ok().flatten();
                                if let (Some(pn), Some(mn)) = (&partner_name, &my_name) {
                                    let _ = state_clone.db.mark_dms_read_by_name(pn, mn);
                                } else {
                                    let _ = state_clone.db.mark_dms_read(&partner, &my_key_for_recv);
                                }
                                send_dm_list_update(&state_clone, &my_key_for_recv);
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
    info!("Peer disconnected: {my_key}");
    let _ = state.broadcast_tx.send(RelayMessage::PeerLeft {
        public_key: my_key,
    });

    // Broadcast updated full user list to all clients.
    broadcast_full_user_list(&state).await;

    // Auto-lockdown with grace period: if no admins/mods remain, wait 30s
    // before locking down. Prevents false lockdowns during deploy restarts.
    if disconnected_role == "admin" || disconnected_role == "mod" {
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

/// H-1: Verify an Ed25519 signature. Returns true if valid.
/// Format: sign(content + '\n' + timestamp_string)
fn verify_ed25519_signature(public_key_hex: &str, content: &str, timestamp: u64, sig_hex: &str) -> bool {
    let Ok(pk_bytes) = hex::decode(public_key_hex) else { return false };
    if pk_bytes.len() != 32 { return false; }
    let pk_array: [u8; 32] = match pk_bytes.try_into() {
        Ok(a) => a,
        Err(_) => return false,
    };
    let Ok(verifying_key) = VerifyingKey::from_bytes(&pk_array) else { return false };

    let Ok(sig_bytes) = hex::decode(sig_hex) else { return false };
    if sig_bytes.len() != 64 { return false; }
    let sig_array: [u8; 64] = match sig_bytes.try_into() {
        Ok(a) => a,
        Err(_) => return false,
    };
    let signature = Signature::from_bytes(&sig_array);

    let message = format!("{}\n{}", content, timestamp);
    use ed25519_dalek::Verifier;
    verifying_key.verify(message.as_bytes(), &signature).is_ok()
}

/// Broadcast an updated peer list to all connected clients.
/// WHY: After role changes (verify, mod, etc.) clients need fresh data for badges.
async fn broadcast_peer_list(state: &Arc<RelayState>) {
    let peers: Vec<PeerInfo> = state
        .peers
        .read()
        .await
        .values()
        .map(|p| {
            let role = state.db.get_role(&p.public_key_hex).unwrap_or_default();
            PeerInfo {
                public_key: p.public_key_hex.clone(),
                display_name: p.display_name.clone(),
                role,
                upload_token: None, // Never broadcast tokens to everyone
            }
        })
        .collect();
    let _ = state.broadcast_tx.send(RelayMessage::PeerList { peers, server_version: None });
}

/// Broadcast the full user list (online + offline) to all connected clients.
async fn broadcast_full_user_list(state: &Arc<RelayState>) {
    if let Ok(all_users) = state.db.list_all_users_with_keys() {
        let online_names: std::collections::HashSet<String> = state
            .peers
            .read()
            .await
            .values()
            .filter_map(|p| p.display_name.clone())
            .map(|n| n.to_lowercase())
            .collect();

        let users: Vec<UserInfo> = all_users
            .into_iter()
            .map(|(name, first_key, role, key_count)| {
                let online = online_names.contains(&name.to_lowercase());
                UserInfo { name, public_key: first_key, role, online, key_count }
            })
            .collect();

        let _ = state.broadcast_tx.send(RelayMessage::FullUserList { users });
    }
}

/// Handle moderation commands. Returns a status message for the caller.
async fn handle_mod_command(
    state: &Arc<RelayState>,
    cmd: &str,
    caller_role: &str,
    target_name: &str,
    _caller_key: &str,
) -> String {
    // Resolve target name → public key(s).
    let target_keys = match state.db.keys_for_name(target_name) {
        Ok(keys) if !keys.is_empty() => keys,
        _ => return format!("User '{}' not found.", target_name),
    };

    let is_admin = caller_role == "admin";
    let is_mod = caller_role == "mod" || is_admin;

    match cmd {
        "/kick" => {
            if !is_mod { return "You need moderator permissions.".to_string(); }
            // Mark keys as kicked so their WebSocket loops will close.
            {
                let mut kicked = state.kicked_keys.write().await;
                for key in &target_keys {
                    kicked.insert(key.clone());
                }
            }
            // Disconnect all sessions for this name by broadcasting a kick.
            let kick_msg = RelayMessage::System {
                message: format!("{} was kicked.", target_name),
            };
            let _ = state.broadcast_tx.send(kick_msg);
            // Remove from connected peers.
            let mut peers = state.peers.write().await;
            for key in &target_keys {
                peers.remove(key);
            }
            format!("Kicked {}.", target_name)
        }
        "/ban" => {
            if !is_admin { return "Only admins can ban users.".to_string(); }
            for key in &target_keys {
                if let Err(e) = state.db.set_banned(key, true) {
                    tracing::error!("Failed to ban: {e}");
                }
            }
            // Mark keys as kicked so their WebSocket loops will close.
            {
                let mut kicked = state.kicked_keys.write().await;
                for key in &target_keys {
                    kicked.insert(key.clone());
                }
            }
            // Also kick them.
            let mut peers = state.peers.write().await;
            for key in &target_keys {
                peers.remove(key);
            }
            let ban_msg = RelayMessage::System {
                message: format!("{} was banned.", target_name),
            };
            let _ = state.broadcast_tx.send(ban_msg);
            format!("Banned {}.", target_name)
        }
        "/unban" => {
            if !is_admin { return "Only admins can unban users.".to_string(); }
            for key in &target_keys {
                if let Err(e) = state.db.set_banned(key, false) {
                    tracing::error!("Failed to unban: {e}");
                }
            }
            format!("Unbanned {}.", target_name)
        }
        "/mod" => {
            if !is_admin { return "Only admins can assign moderators.".to_string(); }
            for key in &target_keys {
                if let Err(e) = state.db.set_role(key, "mod") {
                    tracing::error!("Failed to set mod: {e}");
                }
            }
            format!("{} is now a moderator.", target_name)
        }
        "/unmod" => {
            if !is_admin { return "Only admins can remove moderators.".to_string(); }
            for key in &target_keys {
                if let Err(e) = state.db.set_role(key, "user") {
                    tracing::error!("Failed to unmod: {e}");
                }
            }
            format!("{} is no longer a moderator.", target_name)
        }
        "/mute" => {
            if !is_mod { return "You need moderator permissions.".to_string(); }
            for key in &target_keys {
                if let Err(e) = state.db.set_role(key, "muted") {
                    tracing::error!("Failed to mute: {e}");
                }
            }
            format!("{} has been muted.", target_name)
        }
        "/unmute" => {
            if !is_mod { return "You need moderator permissions.".to_string(); }
            for key in &target_keys {
                if let Err(e) = state.db.set_role(key, "user") {
                    tracing::error!("Failed to unmute: {e}");
                }
            }
            format!("{} has been unmuted.", target_name)
        }
        _ => "Unknown moderation command.".to_string(),
    }
}

/// Send an updated DM conversation list to a specific user via broadcast (filtered by send loop).
fn send_dm_list_update(state: &Arc<RelayState>, user_key: &str) {
    match state.db.get_dm_conversations(user_key) {
        Ok(convos) => {
            let conversations: Vec<DmConversationData> = convos.into_iter().map(|c| DmConversationData {
                partner_key: c.partner_key,
                partner_name: c.partner_name,
                last_message: c.last_message,
                last_timestamp: c.last_timestamp,
                unread_count: c.unread_count,
            }).collect();
            let _ = state.broadcast_tx.send(RelayMessage::DmList {
                target: Some(user_key.to_string()),
                conversations,
            });
        }
        Err(e) => {
            tracing::error!("Failed to get DM conversations for {}: {e}", user_key);
        }
    }
}
