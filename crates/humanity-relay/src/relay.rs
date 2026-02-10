//! Core relay logic: connection management and message routing.

use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, RwLock};
use tracing::info;

use crate::storage::Storage;

/// Maximum broadcast channel capacity.
const BROADCAST_CAPACITY: usize = 256;

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
            lockdown: RwLock::new(false),
            auto_lockdown: RwLock::new(false),
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
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub public_key: String,
    pub display_name: Option<String>,
    #[serde(default)]
    pub role: String,
}

/// Handle a single WebSocket connection.
pub async fn handle_connection(socket: WebSocket, state: Arc<RelayState>) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let mut broadcast_rx = state.broadcast_tx.subscribe();
    let mut peer_key: Option<String> = None;

    // Wait for the identify message first.
    while let Some(Ok(msg)) = ws_rx.next().await {
        if let Message::Text(text) = msg {
            if let Ok(RelayMessage::Identify { public_key, display_name, link_code, invite_code }) =
                serde_json::from_str::<RelayMessage>(&text)
            {
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

                let peer = Peer {
                    public_key_hex: public_key.clone(),
                    display_name: final_name.clone(),
                };

                // Register peer.
                state.peers.write().await.insert(public_key.clone(), peer);
                peer_key = Some(public_key.clone());

                info!("Peer connected: {public_key} ({:?})", final_name);

                // Send current peer list to the new peer.
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
                        }
                    })
                    .collect();

                let list_msg = serde_json::to_string(&RelayMessage::PeerList { peers }).unwrap();
                let _ = ws_tx.send(Message::Text(list_msg.into())).await;

                // Send channel list.
                if let Ok(channels) = state.db.list_channels() {
                    let channel_infos: Vec<ChannelInfo> = channels.into_iter().map(|(id, name, desc, ro)| {
                        ChannelInfo { id, name, description: desc, read_only: ro }
                    }).collect();
                    let ch_msg = serde_json::to_string(&RelayMessage::ChannelList { channels: channel_infos }).unwrap();
                    let _ = ws_tx.send(Message::Text(ch_msg.into())).await;
                }

                // Announce to everyone.
                let peer_role = state.db.get_role(&public_key).unwrap_or_default();
                let _ = state.broadcast_tx.send(RelayMessage::PeerJoined {
                    public_key,
                    display_name: final_name,
                    role: peer_role.clone(),
                });

                // Auto-unlock: if an admin/mod connects and lockdown was auto-set, lift it.
                if peer_role == "admin" || peer_role == "mod" {
                    let is_auto = *state.auto_lockdown.read().await;
                    if is_auto {
                        let locked = *state.lockdown.read().await;
                        if locked {
                            *state.lockdown.write().await = false;
                            *state.auto_lockdown.write().await = false;
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
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = broadcast_rx.recv().await {
            // Don't echo chat/typing/delete messages back to the sender.
            let should_skip = match &msg {
                RelayMessage::Chat { from, .. } => from == &my_key_for_broadcast,
                RelayMessage::Typing { from, .. } => from == &my_key_for_broadcast,
                RelayMessage::Delete { from, .. } => from == &my_key_for_broadcast,
                RelayMessage::Reaction { from, .. } => from == &my_key_for_broadcast,
                _ => false,
            };
            if should_skip {
                continue;
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
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
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

                                // Enforce max message length (2000 chars for user text).
                                // Quotes (lines starting with "> ") are exempt.
                                let user_text_len: usize = content.lines()
                                    .filter(|l| !l.starts_with("> "))
                                    .map(|l| l.len() + 1)
                                    .sum();
                                if user_text_len > 2001 {
                                    let private = RelayMessage::Private {
                                        to: my_key_for_recv.clone(),
                                        message: format!("Message too long ({} chars, max 2000). Please shorten it.", user_text_len.saturating_sub(1)),
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
                                            ];
                                            if role == "admin" || role == "mod" {
                                                help_text.push("".to_string());
                                                help_text.push("🛡️ Moderator commands:".to_string());
                                                help_text.push("  /users — List all registered users (online/offline)".to_string());
                                                help_text.push("  /kick <name> — Disconnect a user".to_string());
                                                help_text.push("  /mute <name> — Mute a user".to_string());
                                                help_text.push("  /unmute <name> — Unmute a user".to_string());
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
                                            let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
                                            if role != "admin" && role != "mod" {
                                                let private = RelayMessage::Private { to: my_key_for_recv.clone(), message: "Only admins and mods can list users.".to_string() };
                                                let _ = state_clone.broadcast_tx.send(private);
                                            } else {
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

                                let chat = RelayMessage::Chat {
                                    from: my_key_for_recv.clone(),
                                    from_name: Some(display.clone()),
                                    content: content.clone(),
                                    timestamp,
                                    signature,
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
                            // Typing indicator — broadcast to other peers.
                            RelayMessage::Typing { .. } => {
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
                            // Reaction — broadcast to all peers.
                            RelayMessage::Reaction { target_from, target_timestamp, emoji, .. } => {
                                // Validate emoji: only short strings, no HTML/JS special chars.
                                if emoji.len() > 32
                                    || emoji.contains('\'')
                                    || emoji.contains('"')
                                    || emoji.contains('<')
                                    || emoji.contains('>')
                                    || emoji.contains('\\')
                                    || emoji.contains('&')
                                {
                                    continue; // Silently drop invalid reactions
                                }
                                let peer = state_clone.peers.read().await.get(&my_key_for_recv).cloned();
                                let display = peer.as_ref().and_then(|p| p.display_name.clone());
                                let reaction = RelayMessage::Reaction {
                                    target_from,
                                    target_timestamp,
                                    emoji,
                                    from: my_key_for_recv.clone(),
                                    from_name: display,
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

    // Clean up: remove peer and announce departure.
    let disconnected_role = state.db.get_role(&my_key).unwrap_or_default();
    state.peers.write().await.remove(&my_key);
    info!("Peer disconnected: {my_key}");
    let _ = state.broadcast_tx.send(RelayMessage::PeerLeft {
        public_key: my_key,
    });

    // Auto-lockdown: if no admins/mods remain, enable lockdown automatically.
    if disconnected_role == "admin" || disconnected_role == "mod" {
        let peers = state.peers.read().await;
        let has_staff = peers.values().any(|p| {
            let role = state.db.get_role(&p.public_key_hex).unwrap_or_default();
            role == "admin" || role == "mod"
        });
        drop(peers);

        if !has_staff {
            let already_locked = *state.lockdown.read().await;
            if !already_locked {
                *state.lockdown.write().await = true;
                *state.auto_lockdown.write().await = true;
                let sys = RelayMessage::System {
                    message: "🔒 Auto-lockdown: no moderators online.".to_string(),
                };
                let _ = state.broadcast_tx.send(sys);
            }
        }
    }
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
            }
        })
        .collect();
    let _ = state.broadcast_tx.send(RelayMessage::PeerList { peers });
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
