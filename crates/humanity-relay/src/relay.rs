//! Core relay logic: connection management and message routing.

use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::info;

use crate::storage::Storage;

/// Maximum broadcast channel capacity.
const BROADCAST_CAPACITY: usize = 256;

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
        #[serde(skip_serializing_if = "Option::is_none")]
        link_code: Option<String>,
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
    },

    /// Server announces a peer joined.
    #[serde(rename = "peer_joined")]
    PeerJoined {
        public_key: String,
        display_name: Option<String>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub public_key: String,
    pub display_name: Option<String>,
}

/// Handle a single WebSocket connection.
pub async fn handle_connection(socket: WebSocket, state: Arc<RelayState>) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let mut broadcast_rx = state.broadcast_tx.subscribe();
    let mut peer_key: Option<String> = None;

    // Wait for the identify message first.
    while let Some(Ok(msg)) = ws_rx.next().await {
        if let Message::Text(text) = msg {
            if let Ok(RelayMessage::Identify { public_key, display_name, link_code }) =
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

                // Check name registration (skip for bot keys).
                if !public_key.starts_with("bot_") {
                    if let Some(ref name) = final_name {
                        match state.db.check_name(name, &public_key) {
                            Ok(None) => {
                                // Name is free — register it.
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
                    .map(|p| PeerInfo {
                        public_key: p.public_key_hex.clone(),
                        display_name: p.display_name.clone(),
                    })
                    .collect();

                let list_msg = serde_json::to_string(&RelayMessage::PeerList { peers }).unwrap();
                let _ = ws_tx.send(Message::Text(list_msg.into())).await;

                // Announce to everyone.
                let _ = state.broadcast_tx.send(RelayMessage::PeerJoined {
                    public_key,
                    display_name: final_name,
                });

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
                            RelayMessage::Chat { content, timestamp, signature, .. } => {
                                let peer = state_clone
                                    .peers
                                    .read()
                                    .await
                                    .get(&my_key_for_recv)
                                    .cloned();

                                let display = peer.as_ref()
                                    .and_then(|p| p.display_name.clone())
                                    .unwrap_or_else(|| "Anonymous".to_string());

                                // Handle slash commands.
                                let trimmed = content.trim();
                                if trimmed.starts_with('/') {
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
                                            // List available commands (private, only sender sees it).
                                            let help_text = [
                                                "📖 Available commands:",
                                                "  /help — Show this message",
                                                "  /link — Generate a code to link another device to your name",
                                                "",
                                                "💡 Tips:",
                                                "  • Click any message to quote/reply to it",
                                                "  • **bold**, *italic*, `code`, ~~strikethrough~~ for formatting",
                                            ].join("\n");
                                            let private = RelayMessage::Private {
                                                to: my_key_for_recv.clone(),
                                                message: help_text,
                                            };
                                            let _ = state_clone.broadcast_tx.send(private);
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

                                let chat = RelayMessage::Chat {
                                    from: my_key_for_recv.clone(),
                                    from_name: Some(display.clone()),
                                    content: content.clone(),
                                    timestamp,
                                    signature,
                                };

                                state_clone.broadcast_and_store(chat).await;

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
    state.peers.write().await.remove(&my_key);
    info!("Peer disconnected: {my_key}");
    let _ = state.broadcast_tx.send(RelayMessage::PeerLeft {
        public_key: my_key,
    });
}
