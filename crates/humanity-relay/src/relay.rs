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
            voice_rooms: RwLock::new(HashMap::new()),
            user_statuses: RwLock::new(HashMap::new()),
            last_search_times: std::sync::Mutex::new(HashMap::new()),
            active_stream: RwLock::new(None),
            federation_connections: RwLock::new(HashMap::new()),
            federation_rate: std::sync::Mutex::new(HashMap::new()),
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

fn listing_from_db(l: &crate::storage::MarketplaceListing) -> ListingData {
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
                                if let Some(data) = raw.get("data").and_then(|d| d.as_str()) {
                                    // Validate: must be valid JSON and < 512KB.
                                    if data.len() > 512 * 1024 {
                                        let private = RelayMessage::Private {
                                            to: my_key_for_recv.clone(),
                                            message: "Sync data too large (max 512KB).".to_string(),
                                        };
                                        let _ = state_clone.broadcast_tx.send(private);
                                    } else if serde_json::from_str::<serde_json::Value>(data).is_ok() {
                                        if let Err(e) = state_clone.db.save_user_data(&my_key_for_recv, data) {
                                            tracing::error!("Failed to save user data: {e}");
                                        }
                                        // Send ack (no broadcast — private to sender via direct ws_tx would be ideal,
                                        // but we use the Private message pattern for simplicity).
                                        let private = RelayMessage::Private {
                                            to: my_key_for_recv.clone(),
                                            message: "sync_ack".to_string(),
                                        };
                                        let _ = state_clone.broadcast_tx.send(private);
                                    }
                                }
                                continue;
                            }
                            Some("sync_load") => {
                                match state_clone.db.load_user_data(&my_key_for_recv) {
                                    Ok(Some((data, updated_at))) => {
                                        let resp = serde_json::json!({
                                            "type": "sync_data",
                                            "data": data,
                                            "updated_at": updated_at
                                        });
                                        // Send via broadcast with Private pattern — but sync_data isn't a RelayMessage variant.
                                        // We need to send raw JSON. Use a system message with a special prefix.
                                        let private = RelayMessage::Private {
                                            to: my_key_for_recv.clone(),
                                            message: format!("__sync_data__:{}", resp.to_string()),
                                        };
                                        let _ = state_clone.broadcast_tx.send(private);
                                    }
                                    Ok(None) => {
                                        let private = RelayMessage::Private {
                                            to: my_key_for_recv.clone(),
                                            message: "__sync_data__:null".to_string(),
                                        };
                                        let _ = state_clone.broadcast_tx.send(private);
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to load user data: {e}");
                                    }
                                }
                                continue;
                            }
                            Some("skill_update") => {
                                // User is broadcasting a skill update
                                if let (Some(skill_id), Some(reality_xp), Some(fantasy_xp), Some(level)) = (
                                    raw.get("skill_id").and_then(|v| v.as_str()),
                                    raw.get("reality_xp").and_then(|v| v.as_f64()),
                                    raw.get("fantasy_xp").and_then(|v| v.as_f64()),
                                    raw.get("level").and_then(|v| v.as_i64()),
                                ) {
                                    let _ = state_clone.db.upsert_skill(&my_key_for_recv, skill_id, reality_xp, fantasy_xp, level as i32);
                                }
                                continue;
                            }
                            Some("skill_verify_request") => {
                                // Forward verification request as a DM to target user
                                if let (Some(skill_id), Some(to_name)) = (
                                    raw.get("skill_id").and_then(|v| v.as_str()),
                                    raw.get("to_name").and_then(|v| v.as_str()),
                                ) {
                                    let level = raw.get("level").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let from_name = {
                                        let peers = state_clone.peers.read().await;
                                        peers.get(&my_key_for_recv).and_then(|p| p.display_name.clone()).unwrap_or_else(|| "Someone".to_string())
                                    };
                                    // Find target key by name
                                    if let Ok(Some(true)) = state_clone.db.check_name(to_name, "") {
                                        // Name exists but belongs to someone else — good, find them
                                    }
                                    // Send as system DM via Private message
                                    // Look up the target key from peers
                                    let target_key = {
                                        let peers = state_clone.peers.read().await;
                                        peers.iter().find(|(_, p)| p.display_name.as_deref() == Some(to_name)).map(|(k, _)| k.clone())
                                    };
                                    if let Some(tk) = target_key {
                                        let msg = format!("__skill_verify_req__:{{\"from_key\":\"{}\",\"from_name\":\"{}\",\"skill_id\":\"{}\",\"level\":{}}}", my_key_for_recv, from_name, skill_id, level);
                                        let private = RelayMessage::Private {
                                            to: tk,
                                            message: msg,
                                        };
                                        let _ = state_clone.broadcast_tx.send(private);
                                    }
                                }
                                continue;
                            }
                            Some("skill_verify_response") => {
                                // User is responding to a verification request
                                if let (Some(skill_id), Some(to_key), Some(approved)) = (
                                    raw.get("skill_id").and_then(|v| v.as_str()),
                                    raw.get("to_key").and_then(|v| v.as_str()),
                                    raw.get("approved").and_then(|v| v.as_bool()),
                                ) {
                                    if approved {
                                        let note = raw.get("note").and_then(|v| v.as_str()).unwrap_or("Verified");
                                        let _ = state_clone.db.store_skill_verification(skill_id, &my_key_for_recv, to_key, note);
                                        let from_name = {
                                            let peers = state_clone.peers.read().await;
                                            peers.get(&my_key_for_recv).and_then(|p| p.display_name.clone()).unwrap_or_else(|| "Someone".to_string())
                                        };
                                        let msg = format!("__skill_verify_resp__:{{\"from_key\":\"{}\",\"from_name\":\"{}\",\"skill_id\":\"{}\",\"approved\":true,\"note\":\"{}\"}}", my_key_for_recv, from_name, skill_id, note);
                                        let private = RelayMessage::Private {
                                            to: to_key.to_string(),
                                            message: msg,
                                        };
                                        let _ = state_clone.broadcast_tx.send(private);
                                    }
                                }
                                continue;
                            }
                            _ => {} // Fall through to normal RelayMessage handling
                        }
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
                                    reply_to: reply_to.clone(),
                                    thread_count: None,
                                };

                                // Store in channel-specific table (with reply_to metadata).
                                if let Some(ref rt) = reply_to {
                                    if let Err(e) = state_clone.db.store_message_in_channel_with_reply(&chat, &ch, &rt.from, rt.timestamp) {
                                        tracing::error!("Failed to persist message: {e}");
                                    }
                                } else {
                                    if let Err(e) = state_clone.db.store_message_in_channel(&chat, &ch) {
                                        tracing::error!("Failed to persist message: {e}");
                                    }
                                }
                                // Broadcast to all (clients filter by their active channel).
                                let _ = state_clone.broadcast_tx.send(chat);

                                // Notify webhook for human messages (non-bot keys).
                                if !my_key_for_recv.starts_with("bot_") {
                                    state_clone.notify_webhook(&display, &content);
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
                            RelayMessage::Dm { to, content, encrypted, nonce, .. } => {
                                let peer = state_clone.peers.read().await.get(&my_key_for_recv).cloned();
                                let sender_name = peer.as_ref()
                                    .and_then(|p| p.display_name.clone())
                                    .unwrap_or_else(|| "Anonymous".to_string());

                                // Validate.
                                if content.is_empty() {
                                    continue;
                                }
                                let dm_role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                let dm_char_limit: usize = if dm_role == "admin" { 10_000 } else { 2_000 };
                                if content.len() > dm_char_limit {
                                    let private = RelayMessage::Private {
                                        to: my_key_for_recv.clone(),
                                        message: format!("DM too long (max {} chars).", dm_char_limit),
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
                                // DM permission check: role-based + friendship.
                                let user_role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                if user_role != "admin" && user_role != "mod" && !my_key_for_recv.starts_with("bot_") {
                                    if user_role != "verified" && user_role != "donor" {
                                        let private = RelayMessage::Private {
                                            to: my_key_for_recv.clone(),
                                            message: "🔒 Verify your account to send DMs.".to_string(),
                                        };
                                        let _ = state_clone.broadcast_tx.send(private);
                                        continue;
                                    }
                                    // Verified/Donor: must be friends with target
                                    let are_friends = state_clone.db.are_friends(&my_key_for_recv, &to).unwrap_or(false);
                                    if !are_friends {
                                        let target_name = state_clone.db.name_for_key(&to).ok().flatten().unwrap_or_else(|| "this user".to_string());
                                        let private = RelayMessage::Private {
                                            to: my_key_for_recv.clone(),
                                            message: format!("🔒 You must be friends to DM {target_name}. Use /follow <name> — if they follow you back, you'll be friends."),
                                        };
                                        let _ = state_clone.broadcast_tx.send(private);
                                        continue;
                                    }
                                }

                                // Rate limiting (same Fibonacci backoff as chat).
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
                                if let Err(e) = state_clone.db.store_dm_e2ee(&my_key_for_recv, &sender_name, &to, &content, ts, encrypted, nonce.as_deref()) {
                                    tracing::error!("Failed to store DM: {e}");
                                }

                                // Send to recipient via broadcast (filtered in send loop).
                                let dm_msg = RelayMessage::Dm {
                                    from: my_key_for_recv.clone(),
                                    from_name: Some(sender_name.clone()),
                                    to: to.clone(),
                                    content: content.clone(),
                                    timestamp: ts,
                                    encrypted,
                                    nonce: nonce.clone(),
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
                                            encrypted: r.encrypted,
                                            nonce: r.nonce,
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
                            // Voice call signaling — forward to target peer.
                            RelayMessage::VoiceCall { to, action, .. } => {
                                let peer = state_clone.peers.read().await.get(&my_key_for_recv).cloned();
                                let sender_name = peer.as_ref()
                                    .and_then(|p| p.display_name.clone());
                                // Check target is connected.
                                let target_connected = state_clone.peers.read().await.contains_key(&to);
                                if !target_connected {
                                    let private = RelayMessage::Private {
                                        to: my_key_for_recv.clone(),
                                        message: "User is not online.".to_string(),
                                    };
                                    let _ = state_clone.broadcast_tx.send(private);
                                } else {
                                    let msg = RelayMessage::VoiceCall {
                                        from: my_key_for_recv.clone(),
                                        from_name: sender_name,
                                        to,
                                        action,
                                    };
                                    let _ = state_clone.broadcast_tx.send(msg);
                                }
                            }
                            // WebRTC signaling — forward to target peer.
                            RelayMessage::WebrtcSignal { to, signal_type, data, .. } => {
                                let target_connected = state_clone.peers.read().await.contains_key(&to);
                                if !target_connected {
                                    let private = RelayMessage::Private {
                                        to: my_key_for_recv.clone(),
                                        message: "User is not online.".to_string(),
                                    };
                                    let _ = state_clone.broadcast_tx.send(private);
                                } else {
                                    let msg = RelayMessage::WebrtcSignal {
                                        from: my_key_for_recv.clone(),
                                        to,
                                        signal_type,
                                        data,
                                    };
                                    let _ = state_clone.broadcast_tx.send(msg);
                                }
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
                            RelayMessage::VoiceRoom { action, room_id, room_name } => {
                                let peer = state_clone.peers.read().await.get(&my_key_for_recv).cloned();
                                let display = peer.as_ref().and_then(|p| p.display_name.clone()).unwrap_or_else(|| "Anonymous".to_string());
                                let user_role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                match action.as_str() {
                                    "create" => {
                                        // Admin/mod only.
                                        if user_role != "admin" && user_role != "mod" {
                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins and mods can create voice channels.".to_string() };
                                            let _ = state_clone.broadcast_tx.send(private);
                                        } else {
                                            let rname = room_name.unwrap_or_else(|| format!("{}'s Room", display));
                                            match state_clone.db.create_voice_channel(&rname, &my_key_for_recv) {
                                                Ok(_id) => {
                                                    broadcast_voice_channel_list(&state_clone).await;
                                                }
                                                Err(e) => {
                                                    tracing::error!("Failed to create voice channel: {e}");
                                                }
                                            }
                                        }
                                    }
                                    "join" => {
                                        if let Some(rid) = room_id {
                                            // Verify channel exists in DB.
                                            let id_num: i64 = rid.parse().unwrap_or(0);
                                            if id_num == 0 || !state_clone.db.voice_channel_exists(id_num).unwrap_or(false) {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Voice channel not found.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
                                                let mut rooms = state_clone.voice_rooms.write().await;
                                                let room = rooms.entry(rid.clone()).or_insert_with(|| {
                                                    // Get name from DB.
                                                    let name = state_clone.db.list_voice_channels().ok()
                                                        .and_then(|chs| chs.into_iter().find(|c| c.id == id_num).map(|c| c.name))
                                                        .unwrap_or_else(|| "Voice Channel".to_string());
                                                    VoiceRoom { name, participants: vec![] }
                                                });
                                                if !room.participants.iter().any(|(k, _)| k == &my_key_for_recv) {
                                                    let existing: Vec<(String, String)> = room.participants.clone();
                                                    room.participants.push((my_key_for_recv.clone(), display.clone()));
                                                    drop(rooms);
                                                    // Send "new_participant" signal to existing members.
                                                    for (pk, _) in &existing {
                                                        let _ = state_clone.broadcast_tx.send(RelayMessage::VoiceRoomSignal {
                                                            from: my_key_for_recv.clone(),
                                                            to: pk.clone(),
                                                            room_id: rid.clone(),
                                                            signal_type: "new_participant".to_string(),
                                                            data: serde_json::json!({ "key": my_key_for_recv, "name": display }),
                                                        });
                                                    }
                                                    broadcast_voice_channel_list(&state_clone).await;
                                                } else {
                                                    drop(rooms);
                                                }
                                            }
                                        }
                                    }
                                    "leave" => {
                                        leave_voice_room(&state_clone, &my_key_for_recv).await;
                                    }
                                    "delete" => {
                                        // Admin/mod only.
                                        if user_role != "admin" && user_role != "mod" {
                                            let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins and mods can delete voice channels.".to_string() };
                                            let _ = state_clone.broadcast_tx.send(private);
                                        } else if let Some(rid) = room_id {
                                            let id_num: i64 = rid.parse().unwrap_or(0);
                                            if id_num > 0 {
                                                // Remove from DB.
                                                let _ = state_clone.db.delete_voice_channel(id_num);
                                                // Remove active room.
                                                state_clone.voice_rooms.write().await.remove(&rid);
                                                broadcast_voice_channel_list(&state_clone).await;
                                            }
                                        }
                                    }
                                    "list" => {
                                        broadcast_voice_channel_list(&state_clone).await;
                                    }
                                    _ => {}
                                }
                            }
                            // ── Voice Room WebRTC Signaling ──
                            RelayMessage::VoiceRoomSignal { to, room_id, signal_type, data, .. } => {
                                // Verify both parties are in the same room.
                                let rooms = state_clone.voice_rooms.read().await;
                                let valid = rooms.get(&room_id).map(|r| {
                                    r.participants.iter().any(|(k, _)| k == &my_key_for_recv) &&
                                    r.participants.iter().any(|(k, _)| k == &to)
                                }).unwrap_or(false);
                                drop(rooms);
                                if valid {
                                    let _ = state_clone.broadcast_tx.send(RelayMessage::VoiceRoomSignal {
                                        from: my_key_for_recv.clone(),
                                        to,
                                        room_id,
                                        signal_type,
                                        data,
                                    });
                                }
                            }
                            // ── Project Board: Task List ──
                            RelayMessage::TaskList { } => {
                                let tasks = build_task_list(&state_clone.db);
                                let _ = state_clone.broadcast_tx.send(RelayMessage::TaskListResponse {
                                    target: Some(my_key_for_recv.clone()),
                                    tasks,
                                });
                            }
                            // ── Project Board: Create Task ──
                            RelayMessage::TaskCreate { title, description, status, priority, assignee, labels } => {
                                let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                if role != "admin" && role != "mod" {
                                    let private = RelayMessage::Private {
                                        to: my_key_for_recv.clone(),
                                        message: "Only admins and mods can create tasks.".to_string(),
                                    };
                                    let _ = state_clone.broadcast_tx.send(private);
                                } else {
                                    let valid_statuses = ["backlog", "in_progress", "testing", "done"];
                                    let valid_priorities = ["low", "medium", "high", "critical"];
                                    let s = if valid_statuses.contains(&status.as_str()) { &status } else { "backlog" };
                                    let p = if valid_priorities.contains(&priority.as_str()) { &priority } else { "medium" };
                                    if title.trim().is_empty() || title.len() > 200 {
                                        let private = RelayMessage::Private {
                                            to: my_key_for_recv.clone(),
                                            message: "Task title must be 1-200 characters.".to_string(),
                                        };
                                        let _ = state_clone.broadcast_tx.send(private);
                                    } else if description.len() > 5000 {
                                        let private = RelayMessage::Private {
                                            to: my_key_for_recv.clone(),
                                            message: "Task description too long (max 5000 chars).".to_string(),
                                        };
                                        let _ = state_clone.broadcast_tx.send(private);
                                    } else {
                                        match state_clone.db.create_task(&title, &description, s, p, assignee.as_deref(), &my_key_for_recv, &labels) {
                                            Ok(id) => {
                                                if let Ok(Some(task)) = state_clone.db.get_task(id) {
                                                    let td = TaskData {
                                                        id: task.id, title: task.title, description: task.description,
                                                        status: task.status, priority: task.priority, assignee: task.assignee,
                                                        created_by: task.created_by, created_at: task.created_at,
                                                        updated_at: task.updated_at, position: task.position, labels: task.labels,
                                                        comment_count: 0,
                                                    };
                                                    let _ = state_clone.broadcast_tx.send(RelayMessage::TaskCreated { task: td });
                                                }
                                            }
                                            Err(e) => {
                                                tracing::error!("Failed to create task: {e}");
                                            }
                                        }
                                    }
                                }
                            }
                            // ── Project Board: Update Task ──
                            RelayMessage::TaskUpdate { id, title, description, priority, assignee, labels } => {
                                let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                if role != "admin" && role != "mod" {
                                    let private = RelayMessage::Private {
                                        to: my_key_for_recv.clone(),
                                        message: "Only admins and mods can edit tasks.".to_string(),
                                    };
                                    let _ = state_clone.broadcast_tx.send(private);
                                } else {
                                    let valid_priorities = ["low", "medium", "high", "critical"];
                                    let p = if valid_priorities.contains(&priority.as_str()) { &priority } else { "medium" };
                                    match state_clone.db.update_task(id, &title, &description, p, assignee.as_deref(), &labels) {
                                        Ok(true) => {
                                            if let Ok(Some(task)) = state_clone.db.get_task(id) {
                                                let cc = state_clone.db.get_task_comment_counts().unwrap_or_default();
                                                let td = TaskData {
                                                    id: task.id, title: task.title, description: task.description,
                                                    status: task.status, priority: task.priority, assignee: task.assignee,
                                                    created_by: task.created_by, created_at: task.created_at,
                                                    updated_at: task.updated_at, position: task.position, labels: task.labels,
                                                    comment_count: *cc.get(&task.id).unwrap_or(&0),
                                                };
                                                let _ = state_clone.broadcast_tx.send(RelayMessage::TaskUpdated { task: td });
                                            }
                                        }
                                        Ok(false) => {
                                            let private = RelayMessage::Private {
                                                to: my_key_for_recv.clone(),
                                                message: "Task not found.".to_string(),
                                            };
                                            let _ = state_clone.broadcast_tx.send(private);
                                        }
                                        Err(e) => tracing::error!("Task update error: {e}"),
                                    }
                                }
                            }
                            // ── Project Board: Move Task ──
                            RelayMessage::TaskMove { id, status } => {
                                let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                if role != "admin" && role != "mod" {
                                    let private = RelayMessage::Private {
                                        to: my_key_for_recv.clone(),
                                        message: "Only admins and mods can move tasks.".to_string(),
                                    };
                                    let _ = state_clone.broadcast_tx.send(private);
                                } else {
                                    let valid_statuses = ["backlog", "in_progress", "testing", "done"];
                                    if !valid_statuses.contains(&status.as_str()) {
                                        continue;
                                    }
                                    match state_clone.db.move_task(id, &status) {
                                        Ok(true) => {
                                            let _ = state_clone.broadcast_tx.send(RelayMessage::TaskMoved { id, status });
                                        }
                                        Ok(false) => {
                                            let private = RelayMessage::Private {
                                                to: my_key_for_recv.clone(),
                                                message: "Task not found.".to_string(),
                                            };
                                            let _ = state_clone.broadcast_tx.send(private);
                                        }
                                        Err(e) => tracing::error!("Task move error: {e}"),
                                    }
                                }
                            }
                            // ── Project Board: Delete Task ──
                            RelayMessage::TaskDelete { id } => {
                                let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                if role != "admin" && role != "mod" {
                                    let private = RelayMessage::Private {
                                        to: my_key_for_recv.clone(),
                                        message: "Only admins and mods can delete tasks.".to_string(),
                                    };
                                    let _ = state_clone.broadcast_tx.send(private);
                                } else {
                                    match state_clone.db.delete_task(id) {
                                        Ok(true) => {
                                            let _ = state_clone.broadcast_tx.send(RelayMessage::TaskDeleted { id });
                                        }
                                        Ok(false) => {
                                            let private = RelayMessage::Private {
                                                to: my_key_for_recv.clone(),
                                                message: "Task not found.".to_string(),
                                            };
                                            let _ = state_clone.broadcast_tx.send(private);
                                        }
                                        Err(e) => tracing::error!("Task delete error: {e}"),
                                    }
                                }
                            }
                            // ── Project Board: Add Comment ──
                            RelayMessage::TaskComment { task_id, content } => {
                                let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                let can_comment = role == "admin" || role == "mod" || role == "verified" || role == "donor";
                                if !can_comment {
                                    let private = RelayMessage::Private {
                                        to: my_key_for_recv.clone(),
                                        message: "Only verified users can comment on tasks.".to_string(),
                                    };
                                    let _ = state_clone.broadcast_tx.send(private);
                                } else if content.trim().is_empty() || content.len() > 2000 {
                                    let private = RelayMessage::Private {
                                        to: my_key_for_recv.clone(),
                                        message: "Comment must be 1-2000 characters.".to_string(),
                                    };
                                    let _ = state_clone.broadcast_tx.send(private);
                                } else {
                                    let peer = state_clone.peers.read().await.get(&my_key_for_recv).cloned();
                                    let author_name = peer.as_ref()
                                        .and_then(|p| p.display_name.clone())
                                        .unwrap_or_else(|| "Anonymous".to_string());
                                    match state_clone.db.add_task_comment(task_id, &my_key_for_recv, &author_name, &content) {
                                        Ok(comment_id) => {
                                            let now = std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap_or_default()
                                                .as_millis() as i64;
                                            let comment = TaskCommentData {
                                                id: comment_id,
                                                task_id,
                                                author_key: my_key_for_recv.clone(),
                                                author_name,
                                                content,
                                                created_at: now,
                                            };
                                            let _ = state_clone.broadcast_tx.send(RelayMessage::TaskCommentAdded { task_id, comment });
                                        }
                                        Err(e) => tracing::error!("Task comment error: {e}"),
                                    }
                                }
                            }
                            // ── Project Board: Request Comments ──
                            RelayMessage::TaskCommentsRequest { task_id } => {
                                match state_clone.db.get_task_comments(task_id) {
                                    Ok(records) => {
                                        let comments: Vec<TaskCommentData> = records.into_iter().map(|r| TaskCommentData {
                                            id: r.id, task_id: r.task_id, author_key: r.author_key,
                                            author_name: r.author_name, content: r.content, created_at: r.created_at,
                                        }).collect();
                                        let _ = state_clone.broadcast_tx.send(RelayMessage::TaskCommentsResponse {
                                            target: Some(my_key_for_recv.clone()),
                                            task_id,
                                            comments,
                                        });
                                    }
                                    Err(e) => tracing::error!("Task comments load error: {e}"),
                                }
                            }
                            // ── Follow/Unfollow ──
                            RelayMessage::Follow { target_key } => {
                                if target_key == my_key_for_recv {
                                    let private = RelayMessage::Private {
                                        to: my_key_for_recv.clone(),
                                        message: "You can't follow yourself.".to_string(),
                                    };
                                    let _ = state_clone.broadcast_tx.send(private);
                                    continue;
                                }
                                // Check target is registered
                                let target_name = state_clone.db.name_for_key(&target_key).ok().flatten();
                                if target_name.is_none() {
                                    let private = RelayMessage::Private {
                                        to: my_key_for_recv.clone(),
                                        message: "User not found.".to_string(),
                                    };
                                    let _ = state_clone.broadcast_tx.send(private);
                                    continue;
                                }
                                match state_clone.db.add_follow(&my_key_for_recv, &target_key) {
                                    Ok(true) => {
                                        let my_name = state_clone.db.name_for_key(&my_key_for_recv).ok().flatten().unwrap_or_else(|| "Someone".to_string());
                                        let tname = target_name.unwrap();
                                        let private = RelayMessage::Private {
                                            to: my_key_for_recv.clone(),
                                            message: format!("✅ You are now following {tname}."),
                                        };
                                        let _ = state_clone.broadcast_tx.send(private);
                                        // Broadcast follow update
                                        let _ = state_clone.broadcast_tx.send(RelayMessage::FollowUpdate {
                                            follower_key: my_key_for_recv.clone(),
                                            followed_key: target_key.clone(),
                                            action: "follow".to_string(),
                                        });
                                        // Notify the followed user if online
                                        let private2 = RelayMessage::Private {
                                            to: target_key.clone(),
                                            message: format!("👁️ {my_name} is now following you."),
                                        };
                                        let _ = state_clone.broadcast_tx.send(private2);
                                    }
                                    Ok(false) => {
                                        let private = RelayMessage::Private {
                                            to: my_key_for_recv.clone(),
                                            message: "You are already following this user.".to_string(),
                                        };
                                        let _ = state_clone.broadcast_tx.send(private);
                                    }
                                    Err(e) => tracing::error!("Follow error: {e}"),
                                }
                            }
                            RelayMessage::Unfollow { target_key } => {
                                match state_clone.db.remove_follow(&my_key_for_recv, &target_key) {
                                    Ok(true) => {
                                        let tname = state_clone.db.name_for_key(&target_key).ok().flatten().unwrap_or_else(|| "user".to_string());
                                        let private = RelayMessage::Private {
                                            to: my_key_for_recv.clone(),
                                            message: format!("✅ You unfollowed {tname}."),
                                        };
                                        let _ = state_clone.broadcast_tx.send(private);
                                        let _ = state_clone.broadcast_tx.send(RelayMessage::FollowUpdate {
                                            follower_key: my_key_for_recv.clone(),
                                            followed_key: target_key,
                                            action: "unfollow".to_string(),
                                        });
                                    }
                                    Ok(false) => {
                                        let private = RelayMessage::Private {
                                            to: my_key_for_recv.clone(),
                                            message: "You are not following this user.".to_string(),
                                        };
                                        let _ = state_clone.broadcast_tx.send(private);
                                    }
                                    Err(e) => tracing::error!("Unfollow error: {e}"),
                                }
                            }

                            // ── Friend Codes ──
                            RelayMessage::FriendCodeRequest {} => {
                                let now_ms = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis() as u64;
                                let expires = now_ms + 24 * 60 * 60 * 1000; // 24 hours
                                match state_clone.db.create_friend_code(&my_key_for_recv, expires, 1) {
                                    Ok(code) => {
                                        let _ = state_clone.broadcast_tx.send(RelayMessage::FriendCodeResponse {
                                            code,
                                            target: Some(my_key_for_recv.clone()),
                                        });
                                    }
                                    Err(e) => {
                                        let private = RelayMessage::Private {
                                            to: my_key_for_recv.clone(),
                                            message: format!("❌ {e}"),
                                        };
                                        let _ = state_clone.broadcast_tx.send(private);
                                    }
                                }
                            }
                            RelayMessage::FriendCodeRedeem { code } => {
                                match state_clone.db.redeem_friend_code(&code) {
                                    Ok(Some((owner_key, owner_name))) => {
                                        if owner_key == my_key_for_recv {
                                            let _ = state_clone.broadcast_tx.send(RelayMessage::FriendCodeResult {
                                                success: false,
                                                name: None,
                                                message: "You can't redeem your own friend code.".to_string(),
                                                target: Some(my_key_for_recv.clone()),
                                            });
                                            continue;
                                        }
                                        let my_name = state_clone.db.name_for_key(&my_key_for_recv).ok().flatten().unwrap_or_else(|| "Someone".to_string());
                                        let oname = owner_name.clone().unwrap_or_else(|| "Unknown".to_string());

                                        // Auto-mutual-follow: both directions.
                                        let _ = state_clone.db.add_follow(&my_key_for_recv, &owner_key);
                                        let _ = state_clone.db.add_follow(&owner_key, &my_key_for_recv);

                                        // Broadcast follow updates.
                                        let _ = state_clone.broadcast_tx.send(RelayMessage::FollowUpdate {
                                            follower_key: my_key_for_recv.clone(),
                                            followed_key: owner_key.clone(),
                                            action: "follow".to_string(),
                                        });
                                        let _ = state_clone.broadcast_tx.send(RelayMessage::FollowUpdate {
                                            follower_key: owner_key.clone(),
                                            followed_key: my_key_for_recv.clone(),
                                            action: "follow".to_string(),
                                        });

                                        // Notify redeemer.
                                        let _ = state_clone.broadcast_tx.send(RelayMessage::FriendCodeResult {
                                            success: true,
                                            name: owner_name.clone(),
                                            message: format!("🎉 You are now friends with {oname}!"),
                                            target: Some(my_key_for_recv.clone()),
                                        });

                                        // Notify the code owner if online.
                                        let private = RelayMessage::Private {
                                            to: owner_key.clone(),
                                            message: format!("🎉 {my_name} redeemed your friend code! You are now friends."),
                                        };
                                        let _ = state_clone.broadcast_tx.send(private);
                                    }
                                    Ok(None) => {
                                        let _ = state_clone.broadcast_tx.send(RelayMessage::FriendCodeResult {
                                            success: false,
                                            name: None,
                                            message: "Invalid or expired friend code.".to_string(),
                                            target: Some(my_key_for_recv.clone()),
                                        });
                                    }
                                    Err(e) => {
                                        tracing::error!("Friend code redeem error: {e}");
                                        let _ = state_clone.broadcast_tx.send(RelayMessage::FriendCodeResult {
                                            success: false,
                                            name: None,
                                            message: "Server error while redeeming code.".to_string(),
                                            target: Some(my_key_for_recv.clone()),
                                        });
                                    }
                                }
                            }

                            // ── Marketplace ──
                            RelayMessage::ListingBrowse {} => {
                                let listings = state_clone.db.get_listings(None, None, 200).unwrap_or_default();
                                let data: Vec<ListingData> = listings.iter().map(listing_from_db).collect();
                                let _ = state_clone.broadcast_tx.send(RelayMessage::ListingList {
                                    target: Some(my_key_for_recv.clone()),
                                    listings: data,
                                });
                            }
                            RelayMessage::ListingCreate { id, title, description, category, condition, price, payment_methods, location } => {
                                let user_role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                if user_role != "verified" && user_role != "donor" && user_role != "mod" && user_role != "admin" {
                                    let private = RelayMessage::Private {
                                        to: my_key_for_recv.clone(),
                                        message: "You must be verified to create listings.".to_string(),
                                    };
                                    let _ = state_clone.broadcast_tx.send(private);
                                    continue;
                                }
                                if title.trim().is_empty() || title.len() > 100 {
                                    let private = RelayMessage::Private {
                                        to: my_key_for_recv.clone(),
                                        message: "Listing title must be 1-100 characters.".to_string(),
                                    };
                                    let _ = state_clone.broadcast_tx.send(private);
                                    continue;
                                }
                                let seller_name = state_clone.db.name_for_key(&my_key_for_recv).ok().flatten().unwrap_or_else(|| "Anonymous".to_string());
                                if let Err(e) = state_clone.db.create_listing(&id, &my_key_for_recv, &seller_name, title.trim(), &description, &category, &condition, &price, &payment_methods, &location) {
                                    tracing::error!("Failed to create listing: {e}");
                                    continue;
                                }
                                if let Ok(Some(listing)) = state_clone.db.get_listing_by_id(&id) {
                                    let _ = state_clone.broadcast_tx.send(RelayMessage::ListingNew {
                                        listing: listing_from_db(&listing),
                                    });
                                }
                            }
                            RelayMessage::ListingUpdate { id, title, description, category, condition, price, payment_methods, location, status } => {
                                let user_role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                let is_admin = user_role == "admin" || user_role == "mod";
                                if let Ok(true) = state_clone.db.update_listing(&id, &my_key_for_recv, title.trim(), &description, &category, &condition, &price, &payment_methods, &location, status.as_deref(), is_admin) {
                                    if let Ok(Some(listing)) = state_clone.db.get_listing_by_id(&id) {
                                        let _ = state_clone.broadcast_tx.send(RelayMessage::ListingUpdated {
                                            listing: listing_from_db(&listing),
                                        });
                                    }
                                }
                            }
                            RelayMessage::ListingDelete { id } => {
                                let user_role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                let is_admin = user_role == "admin" || user_role == "mod";
                                if let Ok(true) = state_clone.db.delete_listing(&id, &my_key_for_recv, is_admin) {
                                    let _ = state_clone.broadcast_tx.send(RelayMessage::ListingDeleted { id });
                                }
                            }

                            // ── Group System ──
                            RelayMessage::GroupCreate { name } => {
                                let user_role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                if user_role != "verified" && user_role != "donor" && user_role != "mod" && user_role != "admin" {
                                    let private = RelayMessage::Private {
                                        to: my_key_for_recv.clone(),
                                        message: "You must be verified to create groups.".to_string(),
                                    };
                                    let _ = state_clone.broadcast_tx.send(private);
                                    continue;
                                }
                                if name.trim().is_empty() || name.len() > 50 {
                                    let private = RelayMessage::Private {
                                        to: my_key_for_recv.clone(),
                                        message: "Group name must be 1-50 characters.".to_string(),
                                    };
                                    let _ = state_clone.broadcast_tx.send(private);
                                    continue;
                                }
                                match state_clone.db.create_group(name.trim(), &my_key_for_recv) {
                                    Ok((id, invite_code)) => {
                                        let private = RelayMessage::Private {
                                            to: my_key_for_recv.clone(),
                                            message: format!("✅ Group '{}' created! Invite code: {invite_code}", name.trim()),
                                        };
                                        let _ = state_clone.broadcast_tx.send(private);
                                        // Send updated group list
                                        if let Ok(user_groups) = state_clone.db.get_user_groups(&my_key_for_recv) {
                                            let groups: Vec<GroupData> = user_groups.into_iter().map(|(id, name, invite_code, role)| {
                                                GroupData { id, name, invite_code, role }
                                            }).collect();
                                            let _ = state_clone.broadcast_tx.send(RelayMessage::GroupList {
                                                target: Some(my_key_for_recv.clone()),
                                                groups,
                                            });
                                        }
                                    }
                                    Err(e) => tracing::error!("Group create error: {e}"),
                                }
                            }
                            RelayMessage::GroupJoin { invite_code } => {
                                match state_clone.db.join_group_by_invite(&invite_code, &my_key_for_recv) {
                                    Ok(Some((gid, gname))) => {
                                        let private = RelayMessage::Private {
                                            to: my_key_for_recv.clone(),
                                            message: format!("✅ Joined group '{gname}'."),
                                        };
                                        let _ = state_clone.broadcast_tx.send(private);
                                        // Send updated group list
                                        if let Ok(user_groups) = state_clone.db.get_user_groups(&my_key_for_recv) {
                                            let groups: Vec<GroupData> = user_groups.into_iter().map(|(id, name, invite_code, role)| {
                                                GroupData { id, name, invite_code, role }
                                            }).collect();
                                            let _ = state_clone.broadcast_tx.send(RelayMessage::GroupList {
                                                target: Some(my_key_for_recv.clone()),
                                                groups,
                                            });
                                        }
                                    }
                                    Ok(None) => {
                                        let private = RelayMessage::Private {
                                            to: my_key_for_recv.clone(),
                                            message: "Invalid invite code.".to_string(),
                                        };
                                        let _ = state_clone.broadcast_tx.send(private);
                                    }
                                    Err(e) => tracing::error!("Group join error: {e}"),
                                }
                            }
                            RelayMessage::GroupLeave { group_id } => {
                                match state_clone.db.leave_group(&group_id, &my_key_for_recv) {
                                    Ok(true) => {
                                        let private = RelayMessage::Private {
                                            to: my_key_for_recv.clone(),
                                            message: "✅ Left the group.".to_string(),
                                        };
                                        let _ = state_clone.broadcast_tx.send(private);
                                        // Send updated group list
                                        if let Ok(user_groups) = state_clone.db.get_user_groups(&my_key_for_recv) {
                                            let groups: Vec<GroupData> = user_groups.into_iter().map(|(id, name, invite_code, role)| {
                                                GroupData { id, name, invite_code, role }
                                            }).collect();
                                            let _ = state_clone.broadcast_tx.send(RelayMessage::GroupList {
                                                target: Some(my_key_for_recv.clone()),
                                                groups,
                                            });
                                        }
                                    }
                                    Ok(false) => {
                                        let private = RelayMessage::Private {
                                            to: my_key_for_recv.clone(),
                                            message: "You are not in this group.".to_string(),
                                        };
                                        let _ = state_clone.broadcast_tx.send(private);
                                    }
                                    Err(e) => tracing::error!("Group leave error: {e}"),
                                }
                            }
                            RelayMessage::GroupMsg { group_id, content } => {
                                // Verify membership
                                let is_member = state_clone.db.is_group_member(&group_id, &my_key_for_recv).unwrap_or(false);
                                if !is_member {
                                    let private = RelayMessage::Private {
                                        to: my_key_for_recv.clone(),
                                        message: "You are not a member of this group.".to_string(),
                                    };
                                    let _ = state_clone.broadcast_tx.send(private);
                                    continue;
                                }
                                if content.is_empty() || content.len() > 2000 {
                                    continue;
                                }
                                let sender_name = state_clone.db.name_for_key(&my_key_for_recv).ok().flatten().unwrap_or_else(|| "Anonymous".to_string());
                                let ts = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis() as u64;
                                let _ = state_clone.db.store_group_message(&group_id, &my_key_for_recv, &sender_name, &content, ts);

                                // H-5 fix: Send group messages only to group members
                                if let Ok(members) = state_clone.db.get_group_members(&group_id) {
                                    for (member_key, _role) in members {
                                        let gm = RelayMessage::GroupMessage {
                                            group_id: group_id.clone(),
                                            from: my_key_for_recv.clone(),
                                            from_name: Some(sender_name.clone()),
                                            content: content.clone(),
                                            timestamp: ts,
                                            target: Some(member_key),
                                        };
                                        let _ = state_clone.broadcast_tx.send(gm);
                                    }
                                }
                            }

                            // ── Device management ──
                            RelayMessage::DeviceListRequest {} => {
                                if let Ok(Some(name)) = state_clone.db.name_for_key(&my_key_for_recv) {
                                    match state_clone.db.keys_for_name_detailed(&name) {
                                        Ok(keys) => {
                                            let peers = state_clone.peers.read().await;
                                            let devices: Vec<DeviceInfo> = keys.into_iter().map(|(key, label, reg_at)| {
                                                let is_online = peers.values().any(|p| p.public_key_hex == key);
                                                DeviceInfo {
                                                    is_current: key == my_key_for_recv,
                                                    public_key: key,
                                                    label,
                                                    registered_at: reg_at as u64,
                                                    is_online,
                                                }
                                            }).collect();
                                            drop(peers);
                                            let resp = RelayMessage::DeviceList { devices, target: Some(my_key_for_recv.clone()) };
                                            let _ = state_clone.broadcast_tx.send(resp);
                                        }
                                        Err(e) => {
                                            let _ = state_clone.broadcast_tx.send(RelayMessage::Private {
                                                to: my_key_for_recv.clone(),
                                                message: format!("Failed to load devices: {e}"),
                                            });
                                        }
                                    }
                                }
                            }

                            RelayMessage::DeviceLabel { public_key, label } => {
                                if let Ok(Some(name)) = state_clone.db.name_for_key(&my_key_for_recv) {
                                    let keys = state_clone.db.keys_for_name(&name).unwrap_or_default();
                                    if keys.contains(&public_key) {
                                        let label_trimmed = label.trim();
                                        if label_trimmed.len() > 32 {
                                            let _ = state_clone.broadcast_tx.send(RelayMessage::Private {
                                                to: my_key_for_recv.clone(),
                                                message: "Label must be 32 characters or less.".to_string(),
                                            });
                                        } else {
                                            let _ = state_clone.db.label_key(&name, &public_key, label_trimmed);
                                            // Send updated device list
                                            if let Ok(keys) = state_clone.db.keys_for_name_detailed(&name) {
                                                let peers = state_clone.peers.read().await;
                                                let devices: Vec<DeviceInfo> = keys.into_iter().map(|(key, label, reg_at)| {
                                                    let is_online = peers.values().any(|p| p.public_key_hex == key);
                                                    DeviceInfo {
                                                        is_current: key == my_key_for_recv,
                                                        public_key: key,
                                                        label,
                                                        registered_at: reg_at as u64,
                                                        is_online,
                                                    }
                                                }).collect();
                                                drop(peers);
                                                let _ = state_clone.broadcast_tx.send(RelayMessage::DeviceList { devices, target: Some(my_key_for_recv.clone()) });
                                            }
                                        }
                                    } else {
                                        let _ = state_clone.broadcast_tx.send(RelayMessage::Private {
                                            to: my_key_for_recv.clone(),
                                            message: "That key doesn't belong to you.".to_string(),
                                        });
                                    }
                                }
                            }

                            RelayMessage::DeviceRevoke { key_prefix } => {
                                if let Ok(Some(name)) = state_clone.db.name_for_key(&my_key_for_recv) {
                                    if my_key_for_recv.starts_with(&key_prefix) {
                                        let _ = state_clone.broadcast_tx.send(RelayMessage::Private {
                                            to: my_key_for_recv.clone(),
                                            message: "Cannot revoke your current device. Use another device to revoke this one.".to_string(),
                                        });
                                    } else {
                                        match state_clone.db.revoke_device(&name, &key_prefix) {
                                            Ok(revoked_keys) if !revoked_keys.is_empty() => {
                                                let first = &revoked_keys[0];
                                                let short: String = first.chars().take(16).collect();
                                                let notice = format!("Device revoked: {}…", short);
                                                let _ = state_clone.broadcast_tx.send(RelayMessage::Private {
                                                    to: my_key_for_recv.clone(),
                                                    message: notice,
                                                });
                                                // Send updated device list
                                                if let Ok(keys) = state_clone.db.keys_for_name_detailed(&name) {
                                                    let peers = state_clone.peers.read().await;
                                                    let devices: Vec<DeviceInfo> = keys.into_iter().map(|(key, label, reg_at)| {
                                                        let is_online = peers.values().any(|p| p.public_key_hex == key);
                                                        DeviceInfo {
                                                            is_current: key == my_key_for_recv,
                                                            public_key: key,
                                                            label,
                                                            registered_at: reg_at as u64,
                                                            is_online,
                                                        }
                                                    }).collect();
                                                    drop(peers);
                                                    let _ = state_clone.broadcast_tx.send(RelayMessage::DeviceList { devices, target: Some(my_key_for_recv.clone()) });
                                                }
                                            }
                                            Ok(_) => {
                                                let _ = state_clone.broadcast_tx.send(RelayMessage::Private {
                                                    to: my_key_for_recv.clone(),
                                                    message: "No matching key found for your account.".to_string(),
                                                });
                                            }
                                            Err(e) => {
                                                let _ = state_clone.broadcast_tx.send(RelayMessage::Private {
                                                    to: my_key_for_recv.clone(),
                                                    message: format!("Revoke failed: {e}"),
                                                });
                                            }
                                        }
                                    }
                                }
                            }

                            // ── Federation Phase 2: incoming federation messages ──
                            RelayMessage::FederationHello { server_id, public_key, name, version, timestamp, signature } => {
                                // Verify this server is in our registry and trusted.
                                if let Ok(servers) = state_clone.db.list_federated_servers() {
                                    if let Some(server) = servers.iter().find(|s| s.server_id == server_id || s.url == server_id) {
                                        if server.trust_tier >= 2 {
                                            // Verify signature if we have their public key.
                                            let sig_valid = if let Some(ref stored_pk) = server.public_key {
                                                verify_ed25519_signature(stored_pk, &timestamp.to_string(), timestamp, &signature)
                                            } else {
                                                // First contact — accept and store their key.
                                                let _ = state_clone.db.update_federated_server_info(&server_id, &name, Some(&public_key), false);
                                                true
                                            };
                                            if sig_valid {
                                                // Respond with welcome.
                                                let fed_channels = state_clone.db.get_federated_channels().unwrap_or_default();
                                                let welcome = RelayMessage::FederationWelcome {
                                                    server_id: state_clone.db.get_or_create_server_keypair().map(|(pk, _)| pk).unwrap_or_default(),
                                                    name: std::env::var("SERVER_NAME").unwrap_or_else(|_| "Humanity Relay".to_string()),
                                                    channels: fed_channels,
                                                };
                                                // Broadcast the welcome — the connecting server will receive it.
                                                let _ = state_clone.broadcast_tx.send(welcome);
                                                let _ = state_clone.db.update_federated_server_status(&server_id, "online");
                                                tracing::info!("Federation: accepted hello from {} ({})", name, server_id);
                                            } else {
                                                tracing::warn!("Federation: invalid signature from {}", server_id);
                                            }
                                        } else {
                                            tracing::warn!("Federation: rejected hello from {} — trust tier {} < 2", name, server.trust_tier);
                                        }
                                    }
                                }
                            }
                            RelayMessage::FederatedChat { server_id, server_name, from_name, content, timestamp, channel, signature } => {
                                // Only accept from known, trusted servers.
                                let accepted = if let Ok(servers) = state_clone.db.list_federated_servers() {
                                    servers.iter().any(|s| (s.server_id == server_id || s.url == server_id) && s.trust_tier >= 2)
                                } else { false };

                                if accepted {
                                    // Only deliver to channels that are federated locally.
                                    if state_clone.db.is_channel_federated(&channel).unwrap_or(false) {
                                        // Broadcast to local clients (don't store — lives on origin server).
                                        let federated_msg = RelayMessage::FederatedChat {
                                            server_id, server_name, from_name, content, timestamp, channel, signature,
                                        };
                                        let _ = state_clone.broadcast_tx.send(federated_msg);
                                    }
                                }
                            }
                            RelayMessage::FederationWelcome { server_id, name, channels } => {
                                tracing::info!("Federation: welcome from {} — federated channels: {:?}", name, channels);
                                let _ = state_clone.db.update_federated_server_status(&server_id, "online");
                            }

                            // ── Streaming ──

                            RelayMessage::StreamStart { title, category } => {
                                // Admin-only check.
                                let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                if role != "admin" {
                                    let private = RelayMessage::Private {
                                        to: my_key_for_recv.clone(),
                                        message: "Only admins can stream to the relay.".to_string(),
                                    };
                                    let _ = state_clone.broadcast_tx.send(private);
                                } else {
                                    let now = std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_millis() as u64;
                                    let streamer_name = {
                                        let peers = state_clone.peers.read().await;
                                        peers.get(&my_key_for_recv).and_then(|p| p.display_name.clone()).unwrap_or_else(|| "Unknown".to_string())
                                    };
                                    // Store in DB.
                                    let db_id = state_clone.db.create_stream(&my_key_for_recv, &title, &category).ok();
                                    let stream = ActiveStream {
                                        streamer_key: my_key_for_recv.clone(),
                                        streamer_name: streamer_name.clone(),
                                        title: title.clone(),
                                        category: category.clone(),
                                        started_at: now,
                                        viewer_keys: HashSet::new(),
                                        external_urls: Vec::new(),
                                        db_id,
                                    };
                                    *state_clone.active_stream.write().await = Some(stream);
                                    info!("Stream started by {} ({}): {}", streamer_name, my_key_for_recv, title);
                                    // Broadcast stream info.
                                    let info_msg = RelayMessage::StreamInfo {
                                        active: true,
                                        streamer_name: Some(streamer_name),
                                        streamer_key: Some(my_key_for_recv.clone()),
                                        title: Some(title),
                                        category: Some(category),
                                        viewer_count: 0,
                                        started_at: Some(now),
                                        external_urls: None,
                                    };
                                    let _ = state_clone.broadcast_tx.send(info_msg);
                                }
                            }

                            RelayMessage::StreamStop {} => {
                                let mut stream_lock = state_clone.active_stream.write().await;
                                if let Some(ref stream) = *stream_lock {
                                    // Only the streamer or an admin can stop.
                                    let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                    if stream.streamer_key == my_key_for_recv || role == "admin" {
                                        let viewer_peak = stream.viewer_keys.len() as i64;
                                        if let Some(db_id) = stream.db_id {
                                            let _ = state_clone.db.end_stream(db_id, viewer_peak);
                                        }
                                        info!("Stream stopped by {}", my_key_for_recv);
                                        *stream_lock = None;
                                        drop(stream_lock);
                                        let info_msg = RelayMessage::StreamInfo {
                                            active: false,
                                            streamer_name: None,
                                            streamer_key: None,
                                            title: None,
                                            category: None,
                                            viewer_count: 0,
                                            started_at: None,
                                            external_urls: None,
                                        };
                                        let _ = state_clone.broadcast_tx.send(info_msg);
                                    }
                                }
                            }

                            RelayMessage::StreamOffer { to, data, .. } => {
                                let _ = state_clone.broadcast_tx.send(RelayMessage::StreamOffer {
                                    from: my_key_for_recv.clone(),
                                    to,
                                    data,
                                });
                            }

                            RelayMessage::StreamAnswer { to, data, .. } => {
                                let _ = state_clone.broadcast_tx.send(RelayMessage::StreamAnswer {
                                    from: my_key_for_recv.clone(),
                                    to,
                                    data,
                                });
                            }

                            RelayMessage::StreamIce { to, data, .. } => {
                                let _ = state_clone.broadcast_tx.send(RelayMessage::StreamIce {
                                    from: my_key_for_recv.clone(),
                                    to,
                                    data,
                                });
                            }

                            RelayMessage::StreamViewerJoin { .. } => {
                                info!("Stream viewer join from {}", my_key_for_recv);
                                let mut stream_lock = state_clone.active_stream.write().await;
                                if let Some(ref mut stream) = *stream_lock {
                                    stream.viewer_keys.insert(my_key_for_recv.clone());
                                    let count = stream.viewer_keys.len() as u32;
                                    let streamer_key = stream.streamer_key.clone();
                                    let info_msg = RelayMessage::StreamInfo {
                                        active: true,
                                        streamer_name: Some(stream.streamer_name.clone()),
                                        streamer_key: Some(stream.streamer_key.clone()),
                                        title: Some(stream.title.clone()),
                                        category: Some(stream.category.clone()),
                                        viewer_count: count,
                                        started_at: Some(stream.started_at),
                                        external_urls: Some(stream.external_urls.clone()),
                                    };
                                    drop(stream_lock);
                                    let _ = state_clone.broadcast_tx.send(info_msg);
                                    // Notify streamer about the new viewer so they can create a WebRTC offer
                                    info!("Sending __stream_viewer_ready__ to streamer {} for viewer {}", streamer_key, my_key_for_recv);
                                    let notify = RelayMessage::Private {
                                        to: streamer_key,
                                        message: format!("__stream_viewer_ready__:{}", my_key_for_recv),
                                    };
                                    let _ = state_clone.broadcast_tx.send(notify);
                                }
                            }

                            RelayMessage::StreamViewerLeave { .. } => {
                                let mut stream_lock = state_clone.active_stream.write().await;
                                if let Some(ref mut stream) = *stream_lock {
                                    stream.viewer_keys.remove(&my_key_for_recv);
                                    let count = stream.viewer_keys.len() as u32;
                                    let peak = stream.viewer_keys.len() as i64;
                                    if let Some(db_id) = stream.db_id {
                                        let _ = state_clone.db.update_stream_viewer_peak(db_id, peak);
                                    }
                                    let info_msg = RelayMessage::StreamInfo {
                                        active: true,
                                        streamer_name: Some(stream.streamer_name.clone()),
                                        streamer_key: Some(stream.streamer_key.clone()),
                                        title: Some(stream.title.clone()),
                                        category: Some(stream.category.clone()),
                                        viewer_count: count,
                                        started_at: Some(stream.started_at),
                                        external_urls: Some(stream.external_urls.clone()),
                                    };
                                    drop(stream_lock);
                                    let _ = state_clone.broadcast_tx.send(info_msg);
                                }
                            }

                            RelayMessage::StreamChat { content, source, source_user, .. } => {
                                let now = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis() as u64;
                                let from_name = {
                                    let peers = state_clone.peers.read().await;
                                    peers.get(&my_key_for_recv).and_then(|p| p.display_name.clone())
                                };
                                // Store in DB if stream is active.
                                {
                                    let stream = state_clone.active_stream.read().await;
                                    if let Some(ref s) = *stream {
                                        if let Some(db_id) = s.db_id {
                                            let _ = state_clone.db.store_stream_chat(
                                                db_id, &content,
                                                from_name.as_deref().unwrap_or("Unknown"),
                                                &source,
                                            );
                                        }
                                    }
                                }
                                let chat_msg = RelayMessage::StreamChat {
                                    content,
                                    source,
                                    source_user,
                                    from: Some(my_key_for_recv.clone()),
                                    from_name,
                                    timestamp: now,
                                };
                                let _ = state_clone.broadcast_tx.send(chat_msg);
                            }

                            RelayMessage::StreamInfoRequest {} => {
                                let stream = state_clone.active_stream.read().await;
                                let info_msg = if let Some(ref s) = *stream {
                                    RelayMessage::StreamInfo {
                                        active: true,
                                        streamer_name: Some(s.streamer_name.clone()),
                                        streamer_key: Some(s.streamer_key.clone()),
                                        title: Some(s.title.clone()),
                                        category: Some(s.category.clone()),
                                        viewer_count: s.viewer_keys.len() as u32,
                                        started_at: Some(s.started_at),
                                        external_urls: Some(s.external_urls.clone()),
                                    }
                                } else {
                                    RelayMessage::StreamInfo {
                                        active: false,
                                        streamer_name: None,
                                        streamer_key: None,
                                        title: None,
                                        category: None,
                                        viewer_count: 0,
                                        started_at: None,
                                        external_urls: None,
                                    }
                                };
                                // Send only to requester (use Private-style targeting via broadcast + from filter in send loop).
                                // For simplicity, broadcast it — clients will update their UI.
                                let _ = state_clone.broadcast_tx.send(info_msg);
                            }

                            RelayMessage::StreamSetExternal { urls } => {
                                let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                if role == "admin" {
                                    let mut stream_lock = state_clone.active_stream.write().await;
                                    if let Some(ref mut stream) = *stream_lock {
                                        stream.external_urls = urls;
                                        let info_msg = RelayMessage::StreamInfo {
                                            active: true,
                                            streamer_name: Some(stream.streamer_name.clone()),
                                            streamer_key: Some(stream.streamer_key.clone()),
                                            title: Some(stream.title.clone()),
                                            category: Some(stream.category.clone()),
                                            viewer_count: stream.viewer_keys.len() as u32,
                                            started_at: Some(stream.started_at),
                                            external_urls: Some(stream.external_urls.clone()),
                                        };
                                        drop(stream_lock);
                                        let _ = state_clone.broadcast_tx.send(info_msg);
                                    }
                                }
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

/// Check if an IP address is private/internal (SSRF prevention).
fn is_private_ip(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            v4.is_loopback() || v4.is_private() || v4.is_link_local()
                || v4.octets()[0] == 0 // 0.0.0.0/8
        }
        std::net::IpAddr::V6(v6) => {
            v6.is_loopback() || v6.is_unspecified()
        }
    }
}

/// Fetch and cache a link preview for a URL. Returns None on failure.
async fn fetch_link_preview(state: &Arc<RelayState>, url: &str) -> Option<LinkPreview> {
    // SSRF prevention: only HTTP(S).
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return None;
    }

    // Don't fetch upload URLs from our own server.
    if url.contains("/uploads/") {
        return None;
    }

    // Check cache first.
    if let Ok(Some(cached)) = state.db.get_link_preview(url) {
        return Some(LinkPreview {
            url: cached.url,
            title: cached.title,
            description: cached.description,
            image: cached.image,
            site_name: cached.site_name,
        });
    }

    // DNS resolution + SSRF check.
    if let Ok(parsed) = url::Url::parse(url) {
        if let Some(host) = parsed.host_str() {
            // Try to resolve the hostname to check for private IPs.
            if let Ok(addrs) = tokio::net::lookup_host(format!("{}:{}", host, parsed.port_or_known_default().unwrap_or(80))).await {
                for addr in addrs {
                    if is_private_ip(&addr.ip()) {
                        tracing::debug!("SSRF blocked: {} resolves to private IP {}", url, addr.ip());
                        return None;
                    }
                }
            }
        }
    }

    // Fetch with timeout and redirect limit.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .ok()?;

    let resp = match client.get(url)
        .header("User-Agent", "HumanityRelay/1.0 LinkPreview")
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!("Link preview fetch failed for {}: {}", url, e);
            return None;
        }
    };

    // Only parse HTML responses.
    let content_type = resp.headers().get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !content_type.contains("text/html") {
        return None;
    }

    // Limit body size to 256KB.
    let body = match resp.text().await {
        Ok(b) if b.len() <= 256 * 1024 => b,
        _ => return None,
    };

    // Parse OG tags with simple regex (avoiding heavy HTML parser dependency).
    let og_title = extract_meta(&body, "og:title")
        .or_else(|| extract_tag(&body, "title"));
    let og_desc = extract_meta(&body, "og:description")
        .or_else(|| extract_meta_name(&body, "description"));
    let og_image = extract_meta(&body, "og:image");
    let og_site = extract_meta(&body, "og:site_name");

    // Cache the result.
    let _ = state.db.cache_link_preview(
        url,
        og_title.as_deref(),
        og_desc.as_deref(),
        og_image.as_deref(),
        og_site.as_deref(),
    );

    // Only return if we have at least a title.
    if og_title.is_some() {
        Some(LinkPreview {
            url: url.to_string(),
            title: og_title,
            description: og_desc.map(|d| if d.len() > 300 { format!("{}…", &d[..297]) } else { d }),
            image: og_image,
            site_name: og_site,
        })
    } else {
        None
    }
}

/// Extract OG meta content: <meta property="X" content="Y">
fn extract_meta(html: &str, property: &str) -> Option<String> {
    let pattern = format!(r#"<meta[^>]*property=["']{}["'][^>]*content=["']([^"']*)["']"#, regex::escape(property));
    let re = regex::Regex::new(&pattern).ok()?;
    re.captures(html).map(|c| html_decode(&c[1]))
        .or_else(|| {
            // Also try content before property (some sites do this).
            let pattern2 = format!(r#"<meta[^>]*content=["']([^"']*)["'][^>]*property=["']{}["']"#, regex::escape(property));
            let re2 = regex::Regex::new(&pattern2).ok()?;
            re2.captures(html).map(|c| html_decode(&c[1]))
        })
}

/// Extract meta name content: <meta name="X" content="Y">
fn extract_meta_name(html: &str, name: &str) -> Option<String> {
    let pattern = format!(r#"<meta[^>]*name=["']{}["'][^>]*content=["']([^"']*)["']"#, regex::escape(name));
    let re = regex::Regex::new(&pattern).ok()?;
    re.captures(html).map(|c| html_decode(&c[1]))
}

/// Extract <title>...</title>
fn extract_tag(html: &str, tag: &str) -> Option<String> {
    let pattern = format!(r"<{0}[^>]*>([^<]*)</{0}>", tag);
    let re = regex::Regex::new(&pattern).ok()?;
    re.captures(html).map(|c| html_decode(c[1].trim()))
}

/// Basic HTML entity decoding.
fn html_decode(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
}

/// Broadcast updated channel list (with categories) to all clients.
fn broadcast_channel_list(state: &Arc<RelayState>) {
    let infos = build_channel_list(&state.db);
    let categories: Vec<CategoryInfo> = state.db.list_categories().unwrap_or_default().into_iter()
        .map(|(id, name, pos)| CategoryInfo { id, name, position: pos }).collect();
    let _ = state.broadcast_tx.send(RelayMessage::ChannelList { channels: infos, categories: Some(categories) });
}

/// Build the task list with comment counts (helper).
fn build_task_list(db: &crate::storage::Storage) -> Vec<TaskData> {
    let tasks = db.list_tasks().unwrap_or_default();
    let counts = db.get_task_comment_counts().unwrap_or_default();
    tasks.into_iter().map(|t| TaskData {
        id: t.id, title: t.title, description: t.description, status: t.status,
        priority: t.priority, assignee: t.assignee, created_by: t.created_by,
        created_at: t.created_at, updated_at: t.updated_at, position: t.position,
        labels: t.labels, comment_count: *counts.get(&t.id).unwrap_or(&0),
    }).collect()
}

/// Build ChannelInfo list with category data from the database.
fn build_channel_list(db: &crate::storage::Storage) -> Vec<ChannelInfo> {
    let categories = db.list_categories().unwrap_or_default();
    let cat_map: std::collections::HashMap<i64, String> = categories.into_iter().map(|(id, name, _)| (id, name)).collect();
    let channels = db.list_channels_with_categories().unwrap_or_default();
    channels.into_iter().map(|(id, name, desc, ro, cat_id)| {
        let cat_name = cat_id.and_then(|cid| cat_map.get(&cid).cloned());
        let federated = db.is_channel_federated(&id).unwrap_or(false);
        ChannelInfo { id, name, description: desc, read_only: ro, category_id: cat_id, category_name: cat_name, federated }
    }).collect()
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
    let peers_raw: Vec<Peer> = state
        .peers
        .read()
        .await
        .values()
        .cloned()
        .collect();
    let statuses = state.user_statuses.read().await;
    let peers: Vec<PeerInfo> = peers_raw.into_iter()
        .map(|p| {
            let role = state.db.get_role(&p.public_key_hex).unwrap_or_default();
            let name_lower = p.display_name.as_ref().map(|n| n.to_lowercase()).unwrap_or_default();
            let (user_status, user_status_text) = statuses.get(&name_lower).cloned().unwrap_or(("online".to_string(), String::new()));
            let ecdh_pub = p.ecdh_public.clone().or_else(|| state.db.get_ecdh_public(&p.public_key_hex).ok().flatten());
            PeerInfo {
                public_key: p.public_key_hex.clone(),
                display_name: p.display_name.clone(),
                role,
                upload_token: None,
                status: user_status,
                status_text: user_status_text,
                ecdh_public: ecdh_pub,
            }
        })
        .collect();
    drop(statuses);
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

        let statuses = state.user_statuses.read().await;
        let users: Vec<UserInfo> = all_users
            .into_iter()
            .map(|(name, first_key, role, key_count)| {
                // Bot accounts (bot_ prefix keys) are always shown as online.
                let online = first_key.starts_with("bot_") || online_names.contains(&name.to_lowercase());
                let (user_status, user_status_text) = statuses.get(&name.to_lowercase()).cloned().unwrap_or(("online".to_string(), String::new()));
                let ecdh_pub = state.db.get_ecdh_public(&first_key).ok().flatten();
                UserInfo { name, public_key: first_key, role, online, key_count, status: user_status, status_text: user_status_text, ecdh_public: ecdh_pub }
            })
            .collect();
        drop(statuses);

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

/// Short key helper (Rust-side).
fn shortKey_rust(hex: &str) -> String {
    if hex.len() >= 8 { hex[..8].to_string() } else { hex.to_string() }
}

/// Build the voice channel list message (persistent channels + active participants).
async fn build_voice_channel_list_msg(state: &Arc<RelayState>) -> RelayMessage {
    let db_channels = state.db.list_voice_channels().unwrap_or_default();
    let rooms = state.voice_rooms.read().await;
    let channels: Vec<VoiceChannelData> = db_channels.into_iter().map(|vc| {
        let rid = vc.id.to_string();
        let participants = rooms.get(&rid).map(|r| {
            r.participants.iter().map(|(k, n)| VoiceRoomParticipant {
                public_key: k.clone(), display_name: n.clone(), muted: false,
            }).collect()
        }).unwrap_or_default();
        VoiceChannelData { id: vc.id, name: vc.name, participants }
    }).collect();
    drop(rooms);
    RelayMessage::VoiceChannelList { channels }
}

/// Broadcast persistent voice channel list to all connected clients.
async fn broadcast_voice_channel_list(state: &Arc<RelayState>) {
    let msg = build_voice_channel_list_msg(state).await;
    let _ = state.broadcast_tx.send(msg);
}

/// Broadcast voice room state to all connected clients (legacy, kept for compatibility).
async fn broadcast_voice_rooms(state: &Arc<RelayState>) {
    // Now we broadcast the persistent voice channel list instead.
    broadcast_voice_channel_list(state).await;
}

/// Remove a user from any voice room they're in.
async fn leave_voice_room(state: &Arc<RelayState>, key: &str) {
    let mut rooms = state.voice_rooms.write().await;
    let mut empty_rooms = Vec::new();
    for (rid, room) in rooms.iter_mut() {
        room.participants.retain(|(k, _)| k != key);
        if room.participants.is_empty() {
            empty_rooms.push(rid.clone());
        }
    }
    // Remove empty in-memory rooms but keep channels in DB.
    for rid in empty_rooms {
        rooms.remove(&rid);
    }
    drop(rooms);
    broadcast_voice_channel_list(state).await;
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

// ── Federation Phase 2: Server-to-Server Messaging ──

/// Sign a message with the server's Ed25519 key.
fn sign_with_server_key(db: &Storage, message: &str) -> Option<String> {
    let (_, sk_hex) = db.get_or_create_server_keypair().ok()?;
    let sk_bytes = hex::decode(&sk_hex).ok()?;
    if sk_bytes.len() != 32 { return None; }
    let sk_array: [u8; 32] = sk_bytes.try_into().ok()?;
    use ed25519_dalek::{Signer, SigningKey};
    let signing_key = SigningKey::from_bytes(&sk_array);
    let sig = signing_key.sign(message.as_bytes());
    Some(hex::encode(sig.to_bytes()))
}

/// Forward a chat message to all connected federated servers.
async fn forward_to_federation(state: &Arc<RelayState>, channel: &str, from_name: &str, content: &str, timestamp: u64) {
    let (server_id, _) = match state.db.get_or_create_server_keypair() {
        Ok(kp) => kp,
        Err(_) => return,
    };
    let server_name = std::env::var("SERVER_NAME").unwrap_or_else(|_| "Humanity Relay".to_string());

    // Sign the message for authenticity.
    let sig_message = format!("{}\n{}\n{}", content, timestamp, channel);
    let signature = sign_with_server_key(&state.db, &sig_message);

    let federated_msg = RelayMessage::FederatedChat {
        server_id: server_id.clone(),
        server_name,
        from_name: from_name.to_string(),
        content: content.to_string(),
        timestamp,
        channel: channel.to_string(),
        signature,
    };

    let json = match serde_json::to_string(&federated_msg) {
        Ok(j) => j,
        Err(_) => return,
    };

    let connections = state.federation_connections.read().await;
    for conn in connections.values() {
        // Rate limit: max 10 messages per second per server.
        let allow = {
            let mut rate = state.federation_rate.lock().unwrap();
            let times = rate.entry(conn.server_id.clone()).or_default();
            let now = Instant::now();
            times.retain(|t| now.duration_since(*t).as_secs() < 1);
            if times.len() < 10 {
                times.push(now);
                true
            } else {
                false
            }
        };
        if allow {
            let _ = conn.tx.send(json.clone());
        }
    }
}

/// Start outbound WebSocket connections to all verified federated servers.
/// Returns the number of connection attempts initiated.
pub async fn start_federation_connections(state: &Arc<RelayState>) -> usize {
    let servers = match state.db.list_federated_servers() {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let mut count = 0;
    for server in servers {
        if server.trust_tier < 2 { continue; }

        let ws_url = {
            let base = server.url.trim_end_matches('/');
            let ws_base = if base.starts_with("https://") {
                base.replacen("https://", "wss://", 1)
            } else if base.starts_with("http://") {
                base.replacen("http://", "ws://", 1)
            } else {
                continue;
            };
            format!("{}/ws", ws_base)
        };

        let state_clone = state.clone();
        let server_id = server.server_id.clone();
        let server_name = server.name.clone();
        let trust_tier = server.trust_tier;

        tokio::spawn(async move {
            federation_connect_loop(state_clone, server_id, server_name, trust_tier, ws_url).await;
        });
        count += 1;
    }
    count
}

/// Connect to a single federated server with exponential backoff reconnection.
async fn federation_connect_loop(
    state: Arc<RelayState>,
    server_id: String,
    server_name: String,
    trust_tier: i32,
    ws_url: String,
) {
    let mut backoff_secs = 5u64;
    loop {
        tracing::info!("Federation: connecting to {} ({})", server_name, ws_url);
        match tokio_tungstenite::connect_async(&ws_url).await {
            Ok((ws_stream, _)) => {
                backoff_secs = 5; // Reset on successful connect.
                tracing::info!("Federation: connected to {}", server_name);

                let (mut write, mut read) = ws_stream.split();
                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

                // Register the connection.
                {
                    let mut conns = state.federation_connections.write().await;
                    conns.insert(server_id.clone(), FederatedConnection {
                        tx: tx.clone(),
                        server_id: server_id.clone(),
                        server_name: server_name.clone(),
                        trust_tier,
                        connected_at: Instant::now(),
                    });
                }
                let _ = state.db.update_federated_server_status(&server_id, "online");
                broadcast_federation_status(&state).await;

                // Send federation hello.
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let (our_pk, _) = state.db.get_or_create_server_keypair().unwrap_or_default();
                let our_name = std::env::var("SERVER_NAME").unwrap_or_else(|_| "Humanity Relay".to_string());
                let sig = sign_with_server_key(&state.db, &timestamp.to_string()).unwrap_or_default();

                let hello = RelayMessage::FederationHello {
                    server_id: our_pk.clone(),
                    public_key: our_pk.clone(),
                    name: our_name,
                    version: env!("BUILD_VERSION").to_string(),
                    timestamp,
                    signature: sig,
                };
                if let Ok(json) = serde_json::to_string(&hello) {
                    use tokio_tungstenite::tungstenite::Message as TMessage;
                    let _ = write.send(TMessage::Text(json.into())).await;
                }

                // Spawn write pump.
                let write_task = tokio::spawn(async move {
                    use tokio_tungstenite::tungstenite::Message as TMessage;
                    while let Some(msg) = rx.recv().await {
                        if write.send(TMessage::Text(msg.into())).await.is_err() {
                            break;
                        }
                    }
                });

                // Read pump: handle incoming messages from federated server.
                let state_for_read = state.clone();
                let _sid_for_read = server_id.clone();
                let read_task = tokio::spawn(async move {
                    use tokio_tungstenite::tungstenite::Message as TMessage;
                    while let Some(Ok(msg)) = read.next().await {
                        if let TMessage::Text(text) = msg {
                            if let Ok(relay_msg) = serde_json::from_str::<RelayMessage>(&text) {
                                match relay_msg {
                                    RelayMessage::FederatedChat { server_id, server_name, from_name, content, timestamp, channel, signature } => {
                                        // Only deliver to locally federated channels.
                                        if state_for_read.db.is_channel_federated(&channel).unwrap_or(false) {
                                            let _ = state_for_read.broadcast_tx.send(RelayMessage::FederatedChat {
                                                server_id, server_name, from_name, content, timestamp, channel, signature,
                                            });
                                        }
                                    }
                                    RelayMessage::FederationWelcome { server_id, name, channels } => {
                                        tracing::info!("Federation: welcome from {} — channels: {:?}", name, channels);
                                        let _ = state_for_read.db.update_federated_server_status(&server_id, "online");
                                    }
                                    _ => {} // Ignore other message types from federation.
                                }
                            }
                        }
                    }
                });

                // Wait for either to finish (connection dropped).
                tokio::select! {
                    _ = write_task => {}
                    _ = read_task => {}
                }

                // Clean up.
                {
                    let mut conns = state.federation_connections.write().await;
                    conns.remove(&server_id);
                }
                let _ = state.db.update_federated_server_status(&server_id, "disconnected");
                broadcast_federation_status(&state).await;
                tracing::warn!("Federation: disconnected from {}", server_name);
            }
            Err(e) => {
                tracing::warn!("Federation: failed to connect to {} ({}): {}", server_name, ws_url, e);
                let _ = state.db.update_federated_server_status(&server_id, "unreachable");
            }
        }

        // Exponential backoff (cap at 5 minutes).
        tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs)).await;
        backoff_secs = (backoff_secs * 2).min(300);
    }
}

/// Broadcast federation status to all connected clients.
async fn broadcast_federation_status(state: &Arc<RelayState>) {
    let servers = state.db.list_federated_servers().unwrap_or_default();
    let connections = state.federation_connections.read().await;

    let statuses: Vec<FederationServerStatus> = servers.iter().map(|s| {
        let connected = connections.contains_key(&s.server_id);
        FederationServerStatus {
            server_id: s.server_id.clone(),
            name: s.name.clone(),
            connected,
            trust_tier: s.trust_tier,
            peer_count: None,
        }
    }).collect();

    let _ = state.broadcast_tx.send(RelayMessage::FederationStatus { servers: statuses });
}
