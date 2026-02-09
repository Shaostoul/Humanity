//! HTTP API for bot integration.
//!
//! Allows bots to send and receive messages without a WebSocket connection.
//! This is the bridge point for AI integration.
//!
//! Endpoints:
//! - POST /api/send    — send a chat message as a bot
//! - GET  /api/messages — poll recent message history
//! - GET  /api/peers   — list connected peers

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::relay::{RelayMessage, RelayState, Peer, PeerInfo};

/// Request body for POST /api/send.
#[derive(Debug, Deserialize)]
pub struct SendRequest {
    /// Bot's display name.
    pub from_name: String,
    /// Message content.
    pub content: String,
}

/// Query params for GET /api/messages.
#[derive(Debug, Deserialize)]
pub struct MessagesQuery {
    /// Only return messages after this index.
    pub after: Option<usize>,
    /// Max messages to return (default 50).
    pub limit: Option<usize>,
}

/// Response for GET /api/messages.
#[derive(Debug, Serialize)]
pub struct MessagesResponse {
    pub messages: Vec<RelayMessage>,
    /// The index of the last message — use as `after` for polling.
    pub cursor: usize,
}

/// POST /api/send — send a message as a bot.
pub async fn send_message(
    State(state): State<Arc<RelayState>>,
    Json(req): Json<SendRequest>,
) -> impl IntoResponse {
    let bot_key = format!("bot_{}", req.from_name.to_lowercase().replace(' ', "_"));

    // Ensure bot appears as a peer.
    {
        let mut peers = state.peers.write().await;
        peers.entry(bot_key.clone()).or_insert_with(|| Peer {
            public_key_hex: bot_key.clone(),
            display_name: Some(req.from_name.clone()),
        });
    }

    let chat = RelayMessage::Chat {
        from: bot_key,
        from_name: Some(req.from_name),
        content: req.content,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        signature: None, // Bot messages are unsigned for now.
    };

    state.broadcast_and_store(chat).await;

    StatusCode::OK
}

/// GET /api/messages — poll recent messages from the database.
///
/// The `after` parameter is now a database row ID (not an array index).
/// Use the returned `cursor` as the `after` value for subsequent polls.
pub async fn get_messages(
    State(state): State<Arc<RelayState>>,
    Query(params): Query<MessagesQuery>,
) -> Json<MessagesResponse> {
    let after = params.after.unwrap_or(0) as i64;
    let limit = params.limit.unwrap_or(50).min(200);

    match state.db.load_messages_after(after, limit) {
        Ok((messages, cursor)) => {
            Json(MessagesResponse { messages, cursor: cursor as usize })
        }
        Err(e) => {
            tracing::error!("Failed to load messages: {e}");
            // Fall back to in-memory.
            let history = state.history.read().await;
            let messages: Vec<RelayMessage> = history
                .iter()
                .skip(after as usize)
                .take(limit)
                .cloned()
                .collect();
            let cursor = after as usize + messages.len();
            Json(MessagesResponse { messages, cursor })
        }
    }
}

/// Response for GET /api/stats.
#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub total_messages: i64,
    pub connected_peers: usize,
}

/// GET /api/stats — relay statistics.
pub async fn get_stats(
    State(state): State<Arc<RelayState>>,
) -> Json<StatsResponse> {
    let total = state.db.message_count().unwrap_or(0);
    let peers = state.peers.read().await.len();
    Json(StatsResponse {
        total_messages: total,
        connected_peers: peers,
    })
}

/// GET /api/peers — list connected peers.
pub async fn get_peers(
    State(state): State<Arc<RelayState>>,
) -> Json<Vec<PeerInfo>> {
    let peers = state.peers.read().await;
    let list: Vec<PeerInfo> = peers
        .values()
        .map(|p| PeerInfo {
            public_key: p.public_key_hex.clone(),
            display_name: p.display_name.clone(),
        })
        .collect();
    Json(list)
}
