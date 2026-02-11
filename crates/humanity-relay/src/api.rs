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
    http::{StatusCode, HeaderMap},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::relay::{RelayMessage, RelayState, Peer, PeerInfo};

/// Verify the `Authorization: Bearer <token>` header against the `API_SECRET` env var.
/// Fails closed: if `API_SECRET` is unset or empty, ALL requests are rejected.
fn check_api_auth(headers: &HeaderMap) -> Result<(), (StatusCode, String)> {
    let expected = std::env::var("API_SECRET").unwrap_or_default();
    if expected.is_empty() {
        return Err((StatusCode::UNAUTHORIZED, "API authentication not configured (API_SECRET not set).".into()));
    }
    let provided = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if provided != expected {
        return Err((StatusCode::UNAUTHORIZED, "Invalid or missing API token.".into()));
    }
    Ok(())
}

/// Request body for POST /api/send.
#[derive(Debug, Deserialize)]
pub struct SendRequest {
    /// Bot's display name.
    pub from_name: String,
    /// Message content.
    pub content: String,
    /// Target channel (defaults to "general").
    #[serde(default = "default_general")]
    pub channel: String,
}

fn default_general() -> String { "general".to_string() }

/// Query params for GET /api/messages.
#[derive(Debug, Deserialize)]
pub struct MessagesQuery {
    /// Only return messages after this index.
    pub after: Option<usize>,
    /// Max messages to return (default 50).
    pub limit: Option<usize>,
    /// Channel to fetch messages from (default: general).
    pub channel: Option<String>,
}

/// Response for GET /api/messages.
#[derive(Debug, Serialize)]
pub struct MessagesResponse {
    pub messages: Vec<RelayMessage>,
    /// The index of the last message — use as `after` for polling.
    pub cursor: usize,
}

/// POST /api/send — send a message as a bot (requires API_SECRET auth).
pub async fn send_message(
    State(state): State<Arc<RelayState>>,
    headers: HeaderMap,
    Json(req): Json<SendRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    // Authenticate.
    check_api_auth(&headers)?;

    // Enforce message length limit (same 2000-char cap as WebSocket users).
    if req.content.len() > 2000 {
        return Err((StatusCode::BAD_REQUEST, format!("Message too long ({} chars, max 2000).", req.content.len())));
    }

    let channel = if req.channel.is_empty() { "general".to_string() } else { req.channel };

    // Validate channel exists.
    if !state.db.channel_exists(&channel).unwrap_or(false) {
        return Err((StatusCode::BAD_REQUEST, format!("Channel '{}' does not exist.", channel)));
    }

    // Respect read-only channels for bot messages.
    if state.db.is_channel_read_only(&channel).unwrap_or(false) {
        return Err((StatusCode::FORBIDDEN, format!("Channel '{}' is read-only.", channel)));
    }

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
        signature: None,
        channel: channel.clone(),
    };

    // Store and broadcast.
    if let Err(e) = state.db.store_message_in_channel(&chat, &channel) {
        tracing::error!("Failed to persist bot message: {e}");
    }
    let _ = state.broadcast_tx.send(chat);

    Ok(StatusCode::OK)
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
    let channel = params.channel.as_deref().unwrap_or("general");

    match state.db.load_channel_messages_after(channel, after, limit) {
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

/// Query params for POST /api/upload (optional user key for FIFO tracking).
#[derive(Debug, Deserialize)]
pub struct UploadQuery {
    /// Public key of the uploader (optional — if absent, no FIFO enforcement).
    pub key: Option<String>,
}

/// POST /api/upload — upload a file (images only, max 5MB).
/// Returns a JSON object with the file URL.
/// If `?key=<public_key>` is provided, enforces a per-user 4-image FIFO.
pub async fn upload_file(
    State(state): State<Arc<RelayState>>,
    Query(query): Query<UploadQuery>,
    mut multipart: axum::extract::Multipart,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    const MAX_SIZE: usize = 5 * 1024 * 1024; // 5MB
    const ALLOWED_TYPES: &[&str] = &["image/png", "image/jpeg", "image/gif", "image/webp"];

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (StatusCode::BAD_REQUEST, format!("Multipart error: {e}"))
    })? {
        let content_type = field.content_type().unwrap_or("").to_string();
        if !ALLOWED_TYPES.contains(&content_type.as_str()) {
            return Err((StatusCode::BAD_REQUEST, format!("Unsupported file type: {}. Allowed: png, jpeg, gif, webp", content_type)));
        }

        let filename = field.file_name().unwrap_or("upload").to_string();
        let data = field.bytes().await.map_err(|e| {
            (StatusCode::BAD_REQUEST, format!("Failed to read file: {e}"))
        })?;

        if data.len() > MAX_SIZE {
            return Err((StatusCode::BAD_REQUEST, format!("File too large ({} bytes, max {})", data.len(), MAX_SIZE)));
        }

        // Generate unique filename.
        let ext = match content_type.as_str() {
            "image/png" => "png",
            "image/jpeg" => "jpg",
            "image/gif" => "gif",
            "image/webp" => "webp",
            _ => "bin",
        };
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let safe_name: String = filename.chars()
            .filter(|c: &char| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .take(32)
            .collect();
        let unique_name = format!("{}_{}.{}", ts, if safe_name.is_empty() { "file" } else { &safe_name }, ext);

        // Store in data/uploads/.
        let upload_dir = std::path::Path::new("data/uploads");
        std::fs::create_dir_all(upload_dir).map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create upload dir: {e}"))
        })?;

        let file_path = upload_dir.join(&unique_name);
        std::fs::write(&file_path, &data).map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write file: {e}"))
        })?;

        // Track upload per user (FIFO: keep max 4 images per key).
        if let Some(ref public_key) = query.key {
            match state.db.record_upload(public_key, &unique_name) {
                Ok(old_files) => {
                    // Delete old files from disk.
                    for old_file in &old_files {
                        let old_path = upload_dir.join(old_file);
                        if let Err(e) = std::fs::remove_file(&old_path) {
                            tracing::warn!("Failed to delete old upload {}: {e}", old_file);
                        } else {
                            tracing::info!("FIFO cleanup: deleted old upload {}", old_file);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to record upload: {e}");
                }
            }
        }

        let url = format!("/uploads/{}", unique_name);
        return Ok(Json(serde_json::json!({ "url": url, "filename": unique_name, "size": data.len() })));
    }

    Err((StatusCode::BAD_REQUEST, "No file provided.".to_string()))
}

/// GitHub push event payload (subset of fields we care about).
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct GitHubPushEvent {
    #[serde(rename = "ref")]
    pub git_ref: Option<String>,
    pub repository: Option<GitHubRepo>,
    pub pusher: Option<GitHubPusher>,
    #[serde(default)]
    pub commits: Vec<GitHubCommit>,
}

#[derive(Debug, Deserialize)]
pub struct GitHubRepo {
    pub full_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GitHubPusher {
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct GitHubCommit {
    pub message: Option<String>,
    pub url: Option<String>,
}

/// Query params for GitHub webhook (secret as query param since GitHub can't send custom headers).
#[derive(Debug, Deserialize)]
pub struct WebhookQuery {
    /// API secret passed as ?secret=... in the webhook URL.
    #[serde(default)]
    pub secret: String,
}

/// POST /api/github-webhook — receive GitHub push events and announce them.
/// Accepts auth via either:
///   1. Authorization: Bearer <token> header (bot API style)
///   2. ?secret=<token> query param (GitHub webhook style)
pub async fn github_webhook(
    State(state): State<Arc<RelayState>>,
    headers: HeaderMap,
    Query(query): Query<WebhookQuery>,
    Json(payload): Json<GitHubPushEvent>,
) -> Result<StatusCode, (StatusCode, String)> {
    // Authenticate via header OR query param.
    let header_ok = check_api_auth(&headers).is_ok();
    let query_ok = {
        let expected = std::env::var("API_SECRET").unwrap_or_default();
        !expected.is_empty() && !query.secret.is_empty() && query.secret == expected
    };
    if !header_ok && !query_ok {
        return Err((StatusCode::UNAUTHORIZED, "Invalid or missing API token.".into()));
    }
    let repo = payload.repository
        .as_ref()
        .and_then(|r| r.full_name.as_deref())
        .unwrap_or("unknown-repo");
    let pusher = payload.pusher
        .as_ref()
        .and_then(|p| p.name.as_deref())
        .unwrap_or("someone");
    let commit_count = payload.commits.len();

    if commit_count == 0 {
        return Ok(StatusCode::OK);
    }

    let mut lines = vec![
        format!(
            "📦 **{}** — {} new commit{} pushed by {}:",
            repo,
            commit_count,
            if commit_count == 1 { "" } else { "s" },
            pusher
        ),
    ];

    for commit in payload.commits.iter().take(10) {
        let msg = commit.message.as_deref().unwrap_or("(no message)");
        // Only the first line of multi-line commit messages.
        let first_line = msg.lines().next().unwrap_or(msg);
        lines.push(format!("• {}", first_line));
    }

    if commit_count > 10 {
        lines.push(format!("  …and {} more", commit_count - 10));
    }

    let announcement = lines.join("\n");

    // Store as a system-ish message in the announcements channel.
    let bot_key = "bot_github".to_string();
    let chat = RelayMessage::Chat {
        from: bot_key.clone(),
        from_name: Some("GitHub".to_string()),
        content: announcement,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        signature: None,
        channel: "announcements".to_string(),
    };

    // Ensure bot peer exists (for display purposes).
    {
        let mut peers = state.peers.write().await;
        peers.entry(bot_key.clone()).or_insert_with(|| Peer {
            public_key_hex: bot_key,
            display_name: Some("GitHub".to_string()),
        });
    }

    if let Err(e) = state.db.store_message_in_channel(&chat, "announcements") {
        tracing::error!("Failed to persist GitHub webhook message: {e}");
    }
    let _ = state.broadcast_tx.send(chat);

    Ok(StatusCode::OK)
}

/// Query params for GET /api/reactions.
#[derive(Debug, Deserialize)]
pub struct ReactionsQuery {
    /// Channel to fetch reactions from (default: general).
    pub channel: Option<String>,
    /// Max reactions to return (default 500).
    pub limit: Option<usize>,
}

/// Response for GET /api/reactions.
#[derive(Debug, Serialize)]
pub struct ReactionsResponse {
    pub reactions: Vec<ReactionEntry>,
}

#[derive(Debug, Serialize)]
pub struct ReactionEntry {
    pub target_from: String,
    pub target_timestamp: u64,
    pub emoji: String,
    pub reactor_key: String,
    pub reactor_name: String,
}

/// GET /api/reactions — load persisted reactions for a channel.
pub async fn get_reactions(
    State(state): State<Arc<RelayState>>,
    Query(params): Query<ReactionsQuery>,
) -> Json<ReactionsResponse> {
    let channel = params.channel.as_deref().unwrap_or("general");
    let limit = params.limit.unwrap_or(500).min(1000);

    match state.db.load_channel_reactions(channel, limit) {
        Ok(records) => {
            let reactions = records.into_iter().map(|r| ReactionEntry {
                target_from: r.target_from,
                target_timestamp: r.target_timestamp,
                emoji: r.emoji,
                reactor_key: r.reactor_key,
                reactor_name: r.reactor_name,
            }).collect();
            Json(ReactionsResponse { reactions })
        }
        Err(e) => {
            tracing::error!("Failed to load reactions: {e}");
            Json(ReactionsResponse { reactions: vec![] })
        }
    }
}

/// GET /api/peers — list connected peers.
pub async fn get_peers(
    State(state): State<Arc<RelayState>>,
) -> Json<Vec<PeerInfo>> {
    let peers = state.peers.read().await;
    let list: Vec<PeerInfo> = peers
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
    Json(list)
}
