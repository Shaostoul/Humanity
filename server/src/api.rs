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
    extract::{Path, Query, State},
    http::{StatusCode, HeaderMap},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use rand::Rng;
use sha2::{Sha256, Digest};
use crate::relay::{RelayMessage, RelayState, Peer, PeerInfo, SearchResultData};

/// Constant-time byte comparison (M-2: prevent timing attacks on HMAC).
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

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
    if provided.len() != expected.len() || !constant_time_eq(provided.as_bytes(), expected.as_bytes()) {
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

    // Enforce message length limit (10000 for bot API, same as admin).
    if req.content.len() > 10000 {
        return Err((StatusCode::BAD_REQUEST, format!("Message too long ({} chars, max 10000).", req.content.len())));
    }

    let channel = if req.channel.is_empty() { "general".to_string() } else { req.channel };

    // Validate channel exists.
    if !state.db.channel_exists(&channel).unwrap_or(false) {
        return Err((StatusCode::BAD_REQUEST, format!("Channel '{}' does not exist.", channel)));
    }

    // Bot API is authenticated with API_SECRET, so it's trusted — skip read-only check.
    // This allows bots (e.g., Heron) to post to read-only channels like #todo.

    // Strip emoji/special chars from bot key generation (name stays as-is for display).
    let clean_name: String = req.from_name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-' || *c == ' ').collect();
    let bot_key = format!("bot_{}", clean_name.to_lowercase().replace(' ', "_"));

    // Ensure bot is registered in the DB (persistent across restarts).
    if let Err(e) = state.db.register_name(&req.from_name, &bot_key) {
        tracing::warn!("Failed to register bot name: {e}");
    }

    // If the bot's display name changed (e.g., "Heron 🪶" → "Heron"), update the peer entry.
    {
        let peers = state.peers.read().await;
        if let Some(existing) = peers.get(&bot_key) {
            if existing.display_name.as_deref() != Some(&req.from_name) {
                drop(peers);
                state.peers.write().await.entry(bot_key.clone()).and_modify(|p| {
                    p.display_name = Some(req.from_name.clone());
                });
            }
        }
    }

    // Ensure bot appears as a peer.
    {
        let mut peers = state.peers.write().await;
        peers.entry(bot_key.clone()).or_insert_with(|| Peer {
            public_key_hex: bot_key.clone(),
            display_name: Some(req.from_name.clone()),
            upload_token: None,
            ecdh_public: None,
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
        reply_to: None,
        thread_count: None,
        message_id: None,
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
    pub version: &'static str,
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
        version: env!("BUILD_VERSION"),
    })
}

/// Query params for POST /api/upload.
#[derive(Debug, Deserialize)]
pub struct UploadQuery {
    /// Legacy: public key (deprecated, use token).
    pub key: Option<String>,
    /// Per-session upload token (M-4: required for uploads).
    pub token: Option<String>,
}

/// Calculate total size of all files in a directory (non-recursive).
fn dir_total_size(dir: &std::path::Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter_map(|e| e.metadata().ok())
                .filter(|m| m.is_file())
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0)
}

/// POST /api/upload — upload a file (images, audio, video, documents, archives).
/// Returns a JSON object with the file URL, filename, size, and type.
/// Requires `?token=<upload_token>` or `?key=<public_key>`.
/// Enforces a per-user 4-file FIFO for images, separate limits for other types.
pub async fn upload_file(
    State(state): State<Arc<RelayState>>,
    Query(query): Query<UploadQuery>,
    mut multipart: axum::extract::Multipart,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    const MAX_SIZE_DEFAULT: usize = 10 * 1024 * 1024; // 10MB for most files (images, docs)
    const MAX_SIZE_MEDIA: usize = 20 * 1024 * 1024; // 20MB for audio/video
    const ALLOWED_TYPES: &[&str] = &[
        "image/png", "image/jpeg", "image/gif", "image/webp",
        "audio/mpeg", "audio/ogg", "audio/wav", "audio/webm", "audio/mp4",
        "video/mp4", "video/webm", "video/ogg",
        "application/pdf", "text/plain", "text/markdown",
        "application/json", "application/zip",
        "application/gzip", "application/x-tar",
        "application/x-gzip", "application/x-compressed-tar",
        "application/octet-stream", // fallback, validated by extension
    ];
    const ALLOWED_EXTENSIONS: &[&str] = &[
        "png", "jpg", "jpeg", "gif", "webp",
        "mp3", "ogg", "wav", "mp4", "webm",
        "pdf", "txt", "md", "json", "zip", "tar.gz", "gz",
        "blend", "stl", "obj", "gltf", "glb", "svg",
    ];
    const BLOCKED_EXTENSIONS: &[&str] = &[
        "exe", "sh", "bat", "cmd", "msi", "dmg", "app", "com", "scr", "pif",
        "html", "htm", "xhtml", "xml", "js", "mjs",
    ];
    /// Maximum total size of all uploads on disk (default 500MB).
    const MAX_TOTAL_UPLOAD_BYTES: u64 = 500 * 1024 * 1024;

    // M-4: Resolve upload token to public key.
    let public_key = if let Some(ref token) = query.token {
        if token.is_empty() {
            return Err((StatusCode::BAD_REQUEST, "Empty upload token.".into()));
        }
        let tokens = state.upload_tokens.read().await;
        match tokens.get(token) {
            Some(key) => key.clone(),
            None => return Err((StatusCode::FORBIDDEN, "Invalid upload token.".into())),
        }
    } else if let Some(ref k) = query.key {
        // Legacy fallback: accept key param but verify it's connected.
        if k.is_empty() {
            return Err((StatusCode::BAD_REQUEST, "Missing upload token or key.".into()));
        }
        let peers = state.peers.read().await;
        if !peers.contains_key(k) {
            return Err((StatusCode::FORBIDDEN, "Upload denied: key is not connected.".into()));
        }
        k.clone()
    } else {
        return Err((StatusCode::BAD_REQUEST, "Missing required 'token' query parameter.".into()));
    };

    // Only verified/mod/admin/donor may upload files.
    {
        let role = state.db.get_role(&public_key).unwrap_or_default();
        if role.is_empty() {
            return Err((StatusCode::FORBIDDEN, "Upload denied: only verified users can upload files. Ask an admin to verify you.".into()));
        }
    }

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (StatusCode::BAD_REQUEST, format!("Multipart error: {e}"))
    })? {
        let content_type = field.content_type().unwrap_or("application/octet-stream").to_string();
        let filename = field.file_name().unwrap_or("upload").to_string();

        // Get file extension from filename.
        let file_ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
        let is_tar_gz = filename.to_lowercase().ends_with(".tar.gz");

        // Block dangerous executable extensions.
        if BLOCKED_EXTENSIONS.contains(&file_ext.as_str()) {
            return Err((StatusCode::BAD_REQUEST, format!("File type .{} is not allowed.", file_ext)));
        }

        // Validate by either content type or extension.
        let type_ok = ALLOWED_TYPES.contains(&content_type.as_str());
        let ext_ok = ALLOWED_EXTENSIONS.contains(&file_ext.as_str()) || is_tar_gz;
        if !type_ok && !ext_ok {
            return Err((StatusCode::BAD_REQUEST, format!("Unsupported file type: {} (.{})", content_type, file_ext)));
        }

        let data = field.bytes().await.map_err(|e| {
            (StatusCode::BAD_REQUEST, format!("Failed to read file: {e}"))
        })?;

        // Determine file category and max size.
        let is_media = content_type.starts_with("audio/") || content_type.starts_with("video/")
            || ["mp3", "ogg", "wav", "mp4", "webm"].contains(&file_ext.as_str());
        let max_size = if is_media { MAX_SIZE_MEDIA } else { MAX_SIZE_DEFAULT };

        if data.len() > max_size {
            return Err((StatusCode::BAD_REQUEST, format!("File too large ({} bytes, max {})", data.len(), max_size)));
        }

        // Validate magic bytes for images (strict).
        let is_image = content_type.starts_with("image/");
        if is_image {
            let magic_ok = match content_type.as_str() {
                "image/png"  => data.len() >= 4 && &data[..4] == b"\x89PNG",
                "image/jpeg" => data.len() >= 3 && &data[..3] == b"\xFF\xD8\xFF",
                "image/gif"  => data.len() >= 6 && (&data[..6] == b"GIF87a" || &data[..6] == b"GIF89a"),
                "image/webp" => data.len() >= 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP",
                _ => false,
            };
            if !magic_ok {
                return Err((StatusCode::BAD_REQUEST, "File content does not match claimed image type.".to_string()));
            }
        }

        // Determine the file extension for storage.
        let ext = if is_tar_gz {
            "tar.gz"
        } else {
            match content_type.as_str() {
                "image/png" => "png",
                "image/jpeg" => "jpg",
                "image/gif" => "gif",
                "image/webp" => "webp",
                "audio/mpeg" => "mp3",
                "audio/ogg" => "ogg",
                "audio/wav" => "wav",
                "video/mp4" => "mp4",
                "video/webm" => "webm",
                "application/pdf" => "pdf",
                "application/json" => "json",
                "application/zip" => "zip",
                "text/plain" => "txt",
                "text/markdown" => "md",
                _ => if ext_ok { &file_ext } else { "bin" },
            }
        };

        // Determine file type category for the response.
        let file_type = if content_type.starts_with("image/") { "image" }
            else if content_type.starts_with("audio/") || ["mp3", "ogg", "wav"].contains(&ext) { "audio" }
            else if content_type.starts_with("video/") || ["mp4", "webm"].contains(&ext) { "video" }
            else if ["blend", "stl", "obj", "gltf", "glb"].contains(&ext) { "3d_model" }
            else if ["zip", "tar.gz", "gz"].contains(&ext) { "archive" }
            else { "document" };
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

        // Check global disk usage before writing.
        let total_size = dir_total_size(upload_dir);
        if total_size + data.len() as u64 > MAX_TOTAL_UPLOAD_BYTES {
            return Err((
                StatusCode::INSUFFICIENT_STORAGE,
                format!("Upload storage full ({:.1} MB / {:.0} MB). Please try again later.",
                    total_size as f64 / (1024.0 * 1024.0),
                    MAX_TOTAL_UPLOAD_BYTES as f64 / (1024.0 * 1024.0)),
            ));
        }

        let file_path = upload_dir.join(&unique_name);
        std::fs::write(&file_path, &data).map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write file: {e}"))
        })?;

        // Track upload per user (FIFO: keep max 4 images per key).
        match state.db.record_upload(&public_key, &unique_name) {
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

        let url = format!("/uploads/{}", unique_name);
        return Ok(Json(serde_json::json!({ "url": url, "filename": unique_name, "size": data.len(), "type": file_type })));
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

/// POST /api/github-webhook — receive GitHub push events and announce them.
/// M-2: Authenticates via GitHub's HMAC-SHA256 signature (X-Hub-Signature-256 header).
/// Uses WEBHOOK_SECRET env var (separate from API_SECRET).
pub async fn github_webhook(
    State(state): State<Arc<RelayState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<StatusCode, (StatusCode, String)> {
    // M-2: Verify HMAC-SHA256 signature from GitHub.
    // Accepts both GITHUB_WEBHOOK_SECRET and WEBHOOK_SECRET env var names.
    let webhook_secret = std::env::var("GITHUB_WEBHOOK_SECRET")
        .or_else(|_| std::env::var("WEBHOOK_SECRET"))
        .unwrap_or_default();

    if webhook_secret.is_empty() {
        // No secret configured — accept for backward compatibility but warn.
        tracing::warn!("GITHUB_WEBHOOK_SECRET not configured — accepting webhook without signature verification");
    } else {
        let sig_header = headers
            .get("x-hub-signature-256")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("sha256="))
            .unwrap_or("");

        if sig_header.is_empty() {
            return Err((StatusCode::UNAUTHORIZED, "Missing X-Hub-Signature-256 header.".into()));
        }

        // Compute HMAC-SHA256.
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        type HmacSha256 = Hmac<Sha256>;

        let mut mac = HmacSha256::new_from_slice(webhook_secret.as_bytes())
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "HMAC key error.".into()))?;
        mac.update(&body);
        let expected = hex::encode(mac.finalize().into_bytes());

        // Constant-time comparison.
        if expected.len() != sig_header.len() || !constant_time_eq(expected.as_bytes(), sig_header.as_bytes()) {
            return Err((StatusCode::UNAUTHORIZED, "Invalid webhook signature.".into()));
        }
    }

    let payload: GitHubPushEvent = serde_json::from_slice(&body)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid JSON: {e}")))?;
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
        reply_to: None,
        thread_count: None,
        message_id: None,
    };

    // Ensure bot peer exists (for display purposes).
    {
        let mut peers = state.peers.write().await;
        peers.entry(bot_key.clone()).or_insert_with(|| Peer {
            public_key_hex: bot_key,
            display_name: Some("GitHub".to_string()),
            upload_token: None,
            ecdh_public: None,
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

/// Query params for GET /api/pins.
#[derive(Debug, Deserialize)]
pub struct PinsQuery {
    /// Channel to fetch pins from (default: general).
    pub channel: Option<String>,
}

/// Response for GET /api/pins.
#[derive(Debug, Serialize)]
pub struct PinsResponse {
    pub pins: Vec<PinEntry>,
}

#[derive(Debug, Serialize)]
pub struct PinEntry {
    pub from_key: String,
    pub from_name: String,
    pub content: String,
    pub original_timestamp: u64,
    pub pinned_by: String,
    pub pinned_at: u64,
}

/// GET /api/pins — load pinned messages for a channel.
pub async fn get_pins(
    State(state): State<Arc<RelayState>>,
    Query(params): Query<PinsQuery>,
) -> Json<PinsResponse> {
    let channel = params.channel.as_deref().unwrap_or("general");

    match state.db.get_pinned_messages(channel) {
        Ok(records) => {
            let pins = records.into_iter().map(|r| PinEntry {
                from_key: r.from_key,
                from_name: r.from_name,
                content: r.content,
                original_timestamp: r.original_timestamp,
                pinned_by: r.pinned_by,
                pinned_at: r.pinned_at,
            }).collect();
            Json(PinsResponse { pins })
        }
        Err(e) => {
            tracing::error!("Failed to load pins: {e}");
            Json(PinsResponse { pins: vec![] })
        }
    }
}

// ── Project Board API ──

/// Query params for GET /api/tasks.
#[derive(Debug, Deserialize)]
pub struct TasksQuery {
    pub status: Option<String>,
    pub project: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Response for GET /api/tasks.
#[derive(Debug, Serialize)]
pub struct TasksResponse {
    pub tasks: Vec<TaskEntry>,
}

#[derive(Debug, Serialize)]
pub struct TaskEntry {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub assignee: Option<String>,
    pub created_by: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub labels: String,
    pub comment_count: i64,
    pub project: String,
}

/// Request body for POST /api/tasks.
#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_backlog")]
    pub status: String,
    #[serde(default = "default_medium")]
    pub priority: String,
    pub assignee: Option<String>,
    #[serde(default = "default_empty_labels")]
    pub labels: String,
    #[serde(default = "default_project")]
    pub project: String,
}

fn default_backlog() -> String { "backlog".to_string() }
fn default_medium() -> String { "medium".to_string() }
fn default_empty_labels() -> String { "[]".to_string() }
fn default_project() -> String { "default".to_string() }

/// GET /api/tasks — list all project board tasks.
pub async fn get_tasks(
    State(state): State<Arc<RelayState>>,
    Query(params): Query<TasksQuery>,
) -> Json<TasksResponse> {
    let tasks = if let Some(ref proj) = params.project {
        state.db.list_tasks_by_project(proj).unwrap_or_default()
    } else {
        state.db.list_tasks().unwrap_or_default()
    };
    let counts = state.db.get_task_comment_counts().unwrap_or_default();
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);
    let entries: Vec<TaskEntry> = tasks.into_iter()
        .filter(|t| params.status.as_ref().map_or(true, |s| &t.status == s))
        .skip(offset)
        .take(limit)
        .map(|t| {
            let cc = *counts.get(&t.id).unwrap_or(&0);
            TaskEntry {
                id: t.id, title: t.title, description: t.description,
                status: t.status, priority: t.priority, assignee: t.assignee,
                created_by: t.created_by, created_at: t.created_at,
                updated_at: t.updated_at, labels: t.labels, comment_count: cc,
                project: t.project,
            }
        }).collect();
    Json(TasksResponse { tasks: entries })
}

/// POST /api/tasks — create a task via bot API (requires API_SECRET auth).
pub async fn create_task(
    State(state): State<Arc<RelayState>>,
    headers: HeaderMap,
    Json(req): Json<CreateTaskRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_api_auth(&headers)?;

    if req.title.trim().is_empty() || req.title.len() > 200 {
        return Err((StatusCode::BAD_REQUEST, "Title must be 1-200 characters.".into()));
    }

    let valid_statuses = ["backlog", "in_progress", "testing", "done"];
    let valid_priorities = ["low", "medium", "high", "critical"];
    let status = if valid_statuses.contains(&req.status.as_str()) { &req.status } else { "backlog" };
    let priority = if valid_priorities.contains(&req.priority.as_str()) { &req.priority } else { "medium" };

    let proj = if req.project.is_empty() { "default" } else { &req.project };
    match state.db.create_task_in_project(&req.title, &req.description, status, priority, req.assignee.as_deref(), "bot_api", &req.labels, proj) {
        Ok(id) => {
            // Broadcast to WebSocket clients.
            if let Ok(Some(task)) = state.db.get_task(id) {
                let td = crate::relay::TaskData {
                    id: task.id, title: task.title, description: task.description,
                    status: task.status, priority: task.priority, assignee: task.assignee,
                    created_by: task.created_by, created_at: task.created_at,
                    updated_at: task.updated_at, position: task.position, labels: task.labels,
                    comment_count: 0, project: task.project,
                };
                let _ = state.broadcast_tx.send(RelayMessage::TaskCreated { task: td });
            }
            Ok(Json(serde_json::json!({ "id": id, "status": "created" })))
        }
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create task: {e}"))),
    }
}

/// Request body for PATCH /api/tasks/:id.
#[derive(Debug, Deserialize)]
pub struct UpdateTaskRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub assignee: Option<String>,
    pub labels: Option<String>,
    pub project: Option<String>,
}

/// PATCH /api/tasks/:id — update a task via bot API (requires API_SECRET auth).
pub async fn update_task(
    State(state): State<Arc<RelayState>>,
    headers: HeaderMap,
    axum::extract::Path(task_id): axum::extract::Path<i64>,
    Json(req): Json<UpdateTaskRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_api_auth(&headers)?;

    // Get existing task.
    let existing = state.db.get_task(task_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Task {} not found.", task_id)))?;

    // Handle status change via move_task.
    if let Some(ref new_status) = req.status {
        let valid_statuses = ["backlog", "in_progress", "testing", "done"];
        if !valid_statuses.contains(&new_status.as_str()) {
            return Err((StatusCode::BAD_REQUEST, format!("Invalid status '{}'. Must be one of: backlog, in_progress, testing, done.", new_status)));
        }
        if new_status != &existing.status {
            state.db.move_task(task_id, new_status)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to move task: {e}")))?;
        }
    }

    // Handle field updates via update_task.
    let title = req.title.as_deref().unwrap_or(&existing.title);
    let description = req.description.as_deref().unwrap_or(&existing.description);
    let priority = req.priority.as_deref().unwrap_or(&existing.priority);
    let assignee = req.assignee.as_deref().or(existing.assignee.as_deref());
    let labels = req.labels.as_deref().unwrap_or(&existing.labels);
    let project = req.project.as_deref().unwrap_or(&existing.project);

    if title.is_empty() || title.len() > 200 {
        return Err((StatusCode::BAD_REQUEST, "Title must be 1-200 characters.".into()));
    }

    let valid_priorities = ["low", "medium", "high", "critical"];
    if !valid_priorities.contains(&priority) {
        return Err((StatusCode::BAD_REQUEST, format!("Invalid priority '{}'.", priority)));
    }

    // Only call update if non-status fields changed.
    let fields_changed = req.title.is_some() || req.description.is_some() || req.priority.is_some() || req.assignee.is_some() || req.labels.is_some() || req.project.is_some();
    if fields_changed {
        state.db.update_task_with_project(task_id, title, description, priority, assignee, labels, project)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to update task: {e}")))?;
    }

    // Return updated task.
    let updated = state.db.get_task(task_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?
        .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, "Task disappeared after update.".into()))?;

    // Broadcast update to WebSocket clients.
    let counts = state.db.get_task_comment_counts().unwrap_or_default();
    let cc = *counts.get(&task_id).unwrap_or(&0);
    let td = crate::relay::TaskData {
        id: updated.id, title: updated.title.clone(), description: updated.description.clone(),
        status: updated.status.clone(), priority: updated.priority.clone(), assignee: updated.assignee.clone(),
        created_by: updated.created_by.clone(), created_at: updated.created_at,
        updated_at: updated.updated_at, position: updated.position, labels: updated.labels.clone(),
        comment_count: cc, project: updated.project.clone(),
    };
    let _ = state.broadcast_tx.send(RelayMessage::TaskUpdated { task: td });

    Ok(Json(serde_json::json!({
        "id": updated.id,
        "title": updated.title,
        "description": updated.description,
        "status": updated.status,
        "priority": updated.priority,
        "assignee": updated.assignee,
        "labels": updated.labels,
        "project": updated.project,
        "updated_at": updated.updated_at,
    })))
}

/// DELETE /api/tasks/:id — delete a task via bot API (requires API_SECRET auth).
pub async fn delete_task(
    State(state): State<Arc<RelayState>>,
    headers: HeaderMap,
    axum::extract::Path(task_id): axum::extract::Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_api_auth(&headers)?;

    match state.db.delete_task(task_id) {
        Ok(true) => {
            let _ = state.broadcast_tx.send(RelayMessage::TaskDeleted { id: task_id });
            Ok(Json(serde_json::json!({ "status": "deleted", "id": task_id })))
        }
        Ok(false) => Err((StatusCode::NOT_FOUND, format!("Task {} not found.", task_id))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to delete task: {e}"))),
    }
}

/// GET /api/tasks/:id/comments — list comments for a task (public).
/// Query params for GET /api/tasks/{id}/comments.
#[derive(Debug, Deserialize)]
pub struct TaskCommentsQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

pub async fn get_task_comments(
    State(state): State<Arc<RelayState>>,
    axum::extract::Path(task_id): axum::extract::Path<i64>,
    Query(params): Query<TaskCommentsQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);
    match state.db.get_task_comments(task_id) {
        Ok(comments) => {
            let list: Vec<serde_json::Value> = comments.into_iter()
                .skip(offset)
                .take(limit)
                .map(|c| serde_json::json!({
                    "id": c.id,
                    "task_id": c.task_id,
                    "author_key": c.author_key,
                    "author_name": c.author_name,
                    "content": c.content,
                    "created_at": c.created_at,
                })).collect();
            Ok(Json(serde_json::json!({ "comments": list })))
        }
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to load comments: {e}"))),
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateCommentRequest {
    pub content: String,
    /// Display name — optional, used when posting via API key (bots).
    #[serde(default)]
    pub author_name: Option<String>,
}

/// POST /api/tasks/:id/comments — add a comment (requires API_SECRET auth).
pub async fn create_task_comment(
    State(state): State<Arc<RelayState>>,
    headers: HeaderMap,
    axum::extract::Path(task_id): axum::extract::Path<i64>,
    Json(req): Json<CreateCommentRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_api_auth(&headers)?;
    if req.content.trim().is_empty() || req.content.len() > 2000 {
        return Err((StatusCode::BAD_REQUEST, "Comment must be 1-2000 characters.".into()));
    }
    let author_name = req.author_name.as_deref().unwrap_or("bot_api");
    match state.db.add_task_comment(task_id, "bot_api", author_name, &req.content) {
        Ok(id) => Ok(Json(serde_json::json!({ "id": id, "status": "created" }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to add comment: {e}"))),
    }
}

// ── Projects API ──

/// Query params for GET /api/projects.
#[derive(Debug, Deserialize)]
pub struct ProjectsQuery {
    pub owner_key: Option<String>,
    pub visibility: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// GET /api/projects — list visible projects (paginated).
pub async fn get_projects(
    State(state): State<Arc<RelayState>>,
    Query(params): Query<ProjectsQuery>,
) -> Json<serde_json::Value> {
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);
    let projects = state.db.get_projects(
        params.visibility.as_deref(),
        params.owner_key.as_deref(),
    ).unwrap_or_default();
    let list: Vec<serde_json::Value> = projects.into_iter()
        .skip(offset)
        .take(limit)
        .map(|(p, tc)| {
            serde_json::json!({
                "id": p.id,
                "name": p.name,
                "description": p.description,
                "owner_key": p.owner_key,
                "visibility": p.visibility,
                "color": p.color,
                "icon": p.icon,
                "created_at": p.created_at,
                "task_count": tc,
            })
        }).collect();
    Json(serde_json::json!(list))
}

/// Request body for POST /api/projects.
#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_public")]
    pub visibility: String,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default = "default_icon")]
    pub icon: String,
}

fn default_public() -> String { "public".to_string() }
fn default_color() -> String { "#4488ff".to_string() }
fn default_icon() -> String { "\u{1F4CB}".to_string() }

/// POST /api/projects — create a project (requires API_SECRET auth).
pub async fn create_project(
    State(state): State<Arc<RelayState>>,
    headers: HeaderMap,
    Json(req): Json<CreateProjectRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_api_auth(&headers)?;

    if req.name.trim().is_empty() || req.name.len() > 100 {
        return Err((StatusCode::BAD_REQUEST, "Project name must be 1-100 characters.".into()));
    }
    if req.description.len() > 2000 {
        return Err((StatusCode::BAD_REQUEST, "Description too long (max 2000 chars).".into()));
    }

    let valid_vis = ["public", "private", "members-only"];
    let vis = if valid_vis.contains(&req.visibility.as_str()) { &req.visibility } else { "public" };

    let id = uuid::Uuid::new_v4().to_string();
    match state.db.create_project(&id, &req.name, &req.description, "bot_api", vis, &req.color, &req.icon) {
        Ok(()) => {
            // Broadcast to WebSocket clients.
            if let Ok(Some(rec)) = state.db.get_project_by_id(&id) {
                let pd = crate::relay::ProjectData {
                    id: rec.id, name: rec.name, description: rec.description,
                    owner_key: rec.owner_key, visibility: rec.visibility,
                    color: rec.color, icon: rec.icon, created_at: rec.created_at,
                    task_count: 0,
                };
                let _ = state.broadcast_tx.send(RelayMessage::ProjectCreated { project: pd });
            }
            Ok(Json(serde_json::json!({ "id": id, "status": "created" })))
        }
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create project: {e}"))),
    }
}

/// Request body for PATCH /api/projects/:id.
#[derive(Debug, Deserialize)]
pub struct UpdateProjectRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub visibility: Option<String>,
    pub color: Option<String>,
    pub icon: Option<String>,
}

/// PATCH /api/projects/:id — update a project (requires API_SECRET auth).
pub async fn update_project(
    State(state): State<Arc<RelayState>>,
    headers: HeaderMap,
    axum::extract::Path(project_id): axum::extract::Path<String>,
    Json(req): Json<UpdateProjectRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_api_auth(&headers)?;

    if project_id == "default" {
        return Err((StatusCode::BAD_REQUEST, "Cannot modify the default project.".into()));
    }

    let existing = state.db.get_project_by_id(&project_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Project '{}' not found.", project_id)))?;

    let name = req.name.as_deref().unwrap_or(&existing.name);
    let description = req.description.as_deref().unwrap_or(&existing.description);
    let visibility = req.visibility.as_deref().unwrap_or(&existing.visibility);
    let color = req.color.as_deref().unwrap_or(&existing.color);
    let icon = req.icon.as_deref().unwrap_or(&existing.icon);

    if name.is_empty() || name.len() > 100 {
        return Err((StatusCode::BAD_REQUEST, "Project name must be 1-100 characters.".into()));
    }

    let valid_vis = ["public", "private", "members-only"];
    if !valid_vis.contains(&visibility) {
        return Err((StatusCode::BAD_REQUEST, format!("Invalid visibility '{}'.", visibility)));
    }

    // Bot API acts as admin.
    state.db.update_project(&project_id, &existing.owner_key, name, description, visibility, color, icon, true)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to update project: {e}")))?;

    // Broadcast.
    let projects = state.db.get_projects(None, None).unwrap_or_default();
    if let Some((rec, tc)) = projects.iter().find(|(r, _)| r.id == project_id) {
        let pd = crate::relay::ProjectData {
            id: rec.id.clone(), name: rec.name.clone(), description: rec.description.clone(),
            owner_key: rec.owner_key.clone(), visibility: rec.visibility.clone(),
            color: rec.color.clone(), icon: rec.icon.clone(), created_at: rec.created_at.clone(),
            task_count: *tc,
        };
        let _ = state.broadcast_tx.send(RelayMessage::ProjectUpdated { project: pd });
    }

    Ok(Json(serde_json::json!({ "id": project_id, "status": "updated" })))
}

/// DELETE /api/projects/:id — delete a project (requires API_SECRET auth).
pub async fn delete_project(
    State(state): State<Arc<RelayState>>,
    headers: HeaderMap,
    axum::extract::Path(project_id): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_api_auth(&headers)?;

    if project_id == "default" {
        return Err((StatusCode::BAD_REQUEST, "Cannot delete the default project.".into()));
    }

    // Bot API acts as admin.
    match state.db.delete_project(&project_id, "bot_api", true) {
        Ok(true) => {
            let _ = state.broadcast_tx.send(RelayMessage::ProjectDeleted { id: project_id.clone() });
            Ok(Json(serde_json::json!({ "status": "deleted", "id": project_id })))
        }
        Ok(false) => Err((StatusCode::NOT_FOUND, format!("Project '{}' not found.", project_id))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to delete project: {e}"))),
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
                upload_token: None, // Never expose tokens via API
                status: "online".to_string(),
                status_text: String::new(),
                ecdh_public: p.ecdh_public.clone(),
            }
        })
        .collect();
    Json(list)
}

// ── Members API ──

/// Query params for GET /api/members.
#[derive(Debug, Deserialize)]
pub struct MembersQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub search: Option<String>,
}

/// Response entry for a server member.
#[derive(Debug, Serialize)]
pub struct MemberEntry {
    pub public_key: String,
    pub name: Option<String>,
    pub role: String,
    pub joined_at: String,
    pub last_seen: Option<String>,
}

/// Response for GET /api/members.
#[derive(Debug, Serialize)]
pub struct MembersResponse {
    pub members: Vec<MemberEntry>,
    pub total: i64,
}

/// GET /api/members — paginated server member directory (public).
pub async fn get_members(
    State(state): State<Arc<RelayState>>,
    Query(params): Query<MembersQuery>,
) -> Json<MembersResponse> {
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);
    let search = params.search.as_deref();
    let members = state.db.get_members(limit, offset, search).unwrap_or_default();
    let total = state.db.get_member_count(search).unwrap_or(0);
    let entries: Vec<MemberEntry> = members.into_iter().map(|m| MemberEntry {
        public_key: m.public_key,
        name: m.name,
        role: m.role,
        joined_at: m.joined_at,
        last_seen: m.last_seen,
    }).collect();
    Json(MembersResponse { members: entries, total })
}

/// GET /api/members/{key} — single member profile with listing count and seller rating.
pub async fn get_member_by_key(
    State(state): State<Arc<RelayState>>,
    axum::extract::Path(key): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let member = state.db.get_member(&key)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let listing_count = state.db.get_seller_listing_count(&key).unwrap_or(0);
    let (avg_rating, review_count) = state.db.get_seller_rating(&key);
    // Get profile data if available.
    let profile = state.db.get_profile_extended(
        member.name.as_deref().unwrap_or("")
    ).ok().flatten();
    let (bio, _socials, avatar_url, _banner_url, _pronouns, location, _website, _privacy) =
        profile.unwrap_or_default();
    Ok(Json(serde_json::json!({
        "public_key": member.public_key,
        "name": member.name,
        "role": member.role,
        "joined_at": member.joined_at,
        "last_seen": member.last_seen,
        "listing_count": listing_count,
        "avg_rating": avg_rating,
        "review_count": review_count,
        "bio": bio,
        "avatar_url": avatar_url,
        "location": location,
    })))
}

/// GET /api/members/count — total member count.
pub async fn get_member_count(
    State(state): State<Arc<RelayState>>,
) -> Json<serde_json::Value> {
    let count = state.db.get_member_count(None).unwrap_or(0);
    Json(serde_json::json!({ "count": count }))
}

// ── Federation API ──

/// Response for GET /api/server-info.
#[derive(Debug, Serialize)]
pub struct ServerInfoResponse {
    pub server_id: String,
    pub name: String,
    pub description: String,
    pub version: &'static str,
    pub channels: Vec<String>,
    pub users_online: usize,
    pub accord_compliant: bool,
    pub public_key: String,
    pub owner_key: String,
    pub member_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub funding: Option<serde_json::Value>,
}

/// GET /api/server-info — public server metadata for federation discovery.
pub async fn get_server_info(
    State(state): State<Arc<RelayState>>,
) -> Json<ServerInfoResponse> {
    let (pk, _) = state.db.get_or_create_server_keypair().unwrap_or_default();
    let channels: Vec<String> = state.db.list_channels()
        .unwrap_or_default()
        .into_iter()
        .map(|(id, _, _, _)| id)
        .collect();
    let users_online = state.peers.read().await.len();
    let member_count = state.db.get_member_count(None).unwrap_or(0);

    // Pull server name/description from config, fall back to env then defaults.
    let config = &state.server_config;
    let server_name = config.get("server_name")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| {
            std::env::var("SERVER_NAME").unwrap_or_else(|_| "Humanity Relay".to_string())
        });
    let server_description = config.get("server_description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let owner_key = config.get("owner_key")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let accord = std::env::var("ACCORD_COMPLIANT").unwrap_or_default() == "true";

    // Include funding config if enabled.
    let funding = config.get("funding").cloned().filter(|f| {
        f.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false)
    });

    Json(ServerInfoResponse {
        server_id: pk.clone(),
        name: server_name,
        description: server_description,
        version: env!("BUILD_VERSION"),
        channels,
        users_online,
        accord_compliant: accord,
        public_key: pk,
        owner_key,
        member_count,
        funding,
    })
}

/// GET /api/civilization — aggregated civilization dashboard stats.
pub async fn get_civilization_stats(
    State(state): State<Arc<RelayState>>,
) -> Json<serde_json::Value> {
    let online_count = state.peers.read().await.len() as u32;
    let stats = state.db.get_civilization_stats(online_count);
    Json(serde_json::to_value(stats).unwrap_or_default())
}

/// Response for GET /api/federation/servers.
#[derive(Debug, Serialize)]
pub struct FederatedServerEntry {
    pub server_id: String,
    pub name: String,
    pub url: String,
    pub public_key: Option<String>,
    pub trust_tier: i32,
    pub accord_compliant: bool,
    pub status: String,
    pub last_seen: Option<i64>,
}

// ── Marketplace API ──

/// Query params for GET /api/listings.
#[derive(Debug, Deserialize)]
pub struct ListingsQuery {
    pub category: Option<String>,
    pub status: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    /// Full-text search query (uses FTS5 when available, LIKE fallback).
    pub q: Option<String>,
}

/// Response for GET /api/listings.
#[derive(Debug, Serialize)]
pub struct ListingsResponse {
    pub listings: Vec<ListingEntry>,
}

#[derive(Debug, Serialize)]
pub struct ListingEntry {
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

/// Image entry in API responses.
#[derive(Debug, Serialize)]
pub struct ListingImageEntry {
    pub id: i64,
    pub url: String,
    pub position: i32,
}

/// GET /api/listings — browse marketplace listings (public).
/// When `q` is provided, uses FTS5 full-text search (falls back to LIKE).
pub async fn get_listings(
    State(state): State<Arc<RelayState>>,
    Query(params): Query<ListingsQuery>,
) -> Json<ListingsResponse> {
    let limit = params.limit.unwrap_or(50).min(200);
    let listings = if let Some(ref q) = params.q {
        if q.trim().is_empty() {
            state.db.get_listings(
                params.category.as_deref(),
                params.status.as_deref().or(Some("active")),
                limit,
            ).unwrap_or_default()
        } else {
            state.db.search_listings(q.trim(), limit).unwrap_or_default()
        }
    } else {
        state.db.get_listings(
            params.category.as_deref(),
            params.status.as_deref().or(Some("active")),
            limit,
        ).unwrap_or_default()
    };
    let offset = params.offset.unwrap_or(0);
    let entries: Vec<ListingEntry> = listings.into_iter()
        .skip(offset)
        .map(|l| ListingEntry {
            id: l.id, seller_key: l.seller_key, seller_name: l.seller_name,
            title: l.title, description: l.description, category: l.category,
            condition: l.condition, price: l.price, payment_methods: l.payment_methods,
            location: l.location, images: l.images, status: l.status, created_at: l.created_at,
            updated_at: l.updated_at,
        }).collect();
    Json(ListingsResponse { listings: entries })
}

/// Request body for POST /api/listings.
#[derive(Debug, Deserialize)]
pub struct CreateListingRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub category: String,
    #[serde(default)]
    pub condition: String,
    #[serde(default)]
    pub price: String,
    #[serde(default)]
    pub payment_methods: String,
    #[serde(default)]
    pub location: String,
}

/// POST /api/listings — create a listing (requires API auth for bots).
pub async fn create_listing(
    State(state): State<Arc<RelayState>>,
    headers: HeaderMap,
    Json(req): Json<CreateListingRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_api_auth(&headers)?;
    if req.title.trim().is_empty() || req.title.len() > 100 {
        return Err((StatusCode::BAD_REQUEST, "Title must be 1-100 characters.".into()));
    }
    let id = format!("api_{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis());
    match state.db.create_listing(&id, "bot_api", "API", req.title.trim(), &req.description, &req.category, &req.condition, &req.price, &req.payment_methods, &req.location) {
        Ok(()) => {
            if let Ok(Some(listing)) = state.db.get_listing_by_id(&id) {
                let _ = state.broadcast_tx.send(crate::relay::RelayMessage::ListingNew {
                    listing: crate::relay::ListingData {
                        id: listing.id.clone(), seller_key: listing.seller_key, seller_name: listing.seller_name,
                        title: listing.title, description: listing.description, category: listing.category,
                        condition: listing.condition, price: listing.price, payment_methods: listing.payment_methods,
                        location: listing.location, images: listing.images, status: listing.status,
                        created_at: listing.created_at, updated_at: listing.updated_at,
                    },
                });
            }
            Ok(Json(serde_json::json!({ "id": id, "status": "created" })))
        }
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed: {e}"))),
    }
}

/// POST /api/listings/{id}/images — upload an image to a listing.
/// Reuses the existing upload infrastructure: caller uploads via /api/upload first,
/// then registers the resulting URL here.
pub async fn add_listing_image(
    State(state): State<Arc<RelayState>>,
    axum::extract::Path(listing_id): axum::extract::Path<String>,
    Query(query): Query<UploadQuery>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Auth: resolve upload token or key to public key.
    let public_key = resolve_upload_key(&state, &query).await?;

    // Verify the caller owns this listing (or is admin).
    let seller_key = state.db.get_listing_seller_key(&listing_id)
        .ok_or((StatusCode::NOT_FOUND, "Listing not found.".into()))?;
    let role = state.db.get_role(&public_key).unwrap_or_default();
    let is_admin = role == "admin" || role == "mod";
    if seller_key != public_key && !is_admin {
        return Err((StatusCode::FORBIDDEN, "You can only add images to your own listings.".into()));
    }

    let url = body.get("url").and_then(|v| v.as_str()).unwrap_or("");
    if url.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Missing 'url' field.".into()));
    }
    let position = body.get("position").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

    match state.db.add_listing_image(&listing_id, url, position) {
        Ok(image_id) => Ok(Json(serde_json::json!({
            "id": image_id,
            "listing_id": listing_id,
            "url": url,
            "position": position,
            "status": "created"
        }))),
        Err(e) => Err((StatusCode::BAD_REQUEST, e)),
    }
}

/// GET /api/listings/{id}/images — get images for a listing.
pub async fn get_listing_images(
    State(state): State<Arc<RelayState>>,
    axum::extract::Path(listing_id): axum::extract::Path<String>,
) -> Json<serde_json::Value> {
    let images = state.db.get_listing_images(&listing_id);
    let entries: Vec<ListingImageEntry> = images.into_iter().map(|img| ListingImageEntry {
        id: img.id,
        url: img.url,
        position: img.position,
    }).collect();
    Json(serde_json::json!({ "images": entries }))
}

/// DELETE /api/listings/{listing_id}/images/{image_id} — remove an image from a listing.
pub async fn delete_listing_image(
    State(state): State<Arc<RelayState>>,
    axum::extract::Path((listing_id, image_id)): axum::extract::Path<(String, i64)>,
    Query(query): Query<UploadQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let public_key = resolve_upload_key(&state, &query).await?;

    let seller_key = state.db.get_listing_seller_key(&listing_id)
        .ok_or((StatusCode::NOT_FOUND, "Listing not found.".into()))?;
    let role = state.db.get_role(&public_key).unwrap_or_default();
    let is_admin = role == "admin" || role == "mod";
    if seller_key != public_key && !is_admin {
        return Err((StatusCode::FORBIDDEN, "You can only delete images from your own listings.".into()));
    }

    match state.db.delete_listing_image(image_id, &listing_id) {
        Ok(true) => Ok(Json(serde_json::json!({ "status": "deleted" }))),
        Ok(false) => Err((StatusCode::NOT_FOUND, "Image not found.".into())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

/// Resolve upload token/key to a public key (shared helper for listing image endpoints).
async fn resolve_upload_key(state: &Arc<RelayState>, query: &UploadQuery) -> Result<String, (StatusCode, String)> {
    if let Some(ref token) = query.token {
        if token.is_empty() {
            return Err((StatusCode::BAD_REQUEST, "Empty upload token.".into()));
        }
        let tokens = state.upload_tokens.read().await;
        match tokens.get(token) {
            Some(key) => Ok(key.clone()),
            None => Err((StatusCode::FORBIDDEN, "Invalid upload token.".into())),
        }
    } else if let Some(ref k) = query.key {
        if k.is_empty() {
            return Err((StatusCode::BAD_REQUEST, "Missing upload token or key.".into()));
        }
        let peers = state.peers.read().await;
        if !peers.contains_key(k) {
            return Err((StatusCode::FORBIDDEN, "Key is not connected.".into()));
        }
        Ok(k.clone())
    } else {
        Err((StatusCode::BAD_REQUEST, "Missing required 'token' or 'key' query parameter.".into()))
    }
}

// ── Reviews API ────────────────────────────────────────────────────────────

/// Query params for GET /api/listings/{id}/reviews.
#[derive(Debug, Deserialize)]
pub struct ReviewsQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub sort: Option<String>,
}

/// GET /api/listings/{id}/reviews — get reviews for a listing (public).
pub async fn get_listing_reviews(
    State(state): State<Arc<RelayState>>,
    axum::extract::Path(listing_id): axum::extract::Path<String>,
    Query(params): Query<ReviewsQuery>,
) -> Json<serde_json::Value> {
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);
    let reviews = state.db.get_reviews(&listing_id, limit + offset).unwrap_or_default();
    let data: Vec<serde_json::Value> = reviews.iter().skip(offset).map(|r| {
        serde_json::json!({
            "id": r.id,
            "listing_id": r.listing_id,
            "reviewer_key": r.reviewer_key,
            "reviewer_name": r.reviewer_name,
            "rating": r.rating,
            "comment": r.comment,
            "created_at": r.created_at,
        })
    }).collect();

    // Also return aggregate info.
    let listing = state.db.get_listing_by_id(&listing_id);
    let (avg, count) = if let Ok(Some(ref l)) = listing {
        state.db.get_seller_rating(&l.seller_key)
    } else {
        (0.0, 0)
    };

    Json(serde_json::json!({
        "reviews": data,
        "avg_rating": avg,
        "review_count": count,
    }))
}

/// Request body for POST /api/listings/{id}/reviews.
#[derive(Debug, Deserialize)]
pub struct CreateReviewRequest {
    pub rating: i32,
    #[serde(default)]
    pub comment: String,
    pub public_key: String,
    pub timestamp: u64,
    pub signature: String,
}

/// POST /api/listings/{id}/reviews — create a review (authenticated via Ed25519 sig).
pub async fn create_listing_review(
    State(state): State<Arc<RelayState>>,
    axum::extract::Path(listing_id): axum::extract::Path<String>,
    Json(body): Json<CreateReviewRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use crate::handlers::broadcast::verify_ed25519_signature;

    // Reject requests older than 5 minutes.
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    if now_ms.saturating_sub(body.timestamp) > 5 * 60 * 1000 {
        return Err((StatusCode::BAD_REQUEST, "Timestamp too old (> 5 min).".into()));
    }

    // Verify signature over "review\n" + listing_id + "\n" + timestamp.
    let sig_content = format!("review\n{}", listing_id);
    if !verify_ed25519_signature(&body.public_key, &sig_content, body.timestamp, &body.signature) {
        return Err((StatusCode::UNAUTHORIZED, "Signature verification failed.".into()));
    }

    let reviewer_name = state.db.name_for_key(&body.public_key)
        .ok().flatten().unwrap_or_else(|| "Anonymous".to_string());

    match state.db.create_review(&listing_id, &body.public_key, &reviewer_name, body.rating, &body.comment) {
        Ok(review_id) => {
            // Broadcast via WebSocket.
            if let Ok(Some(review)) = state.db.get_review_by_id(review_id) {
                let _ = state.broadcast_tx.send(crate::relay::RelayMessage::ReviewCreated {
                    review: crate::relay::review_from_db(&review),
                });
            }
            Ok(Json(serde_json::json!({ "id": review_id, "status": "created" })))
        }
        Err(e) => Err((StatusCode::BAD_REQUEST, e)),
    }
}

/// DELETE /api/listings/{id}/reviews/{review_id} — delete a review.
pub async fn delete_listing_review(
    State(state): State<Arc<RelayState>>,
    axum::extract::Path((listing_id, review_id)): axum::extract::Path<(String, i64)>,
    Query(q): Query<VaultSyncQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use crate::handlers::broadcast::verify_ed25519_signature;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    if now_ms.saturating_sub(q.timestamp) > 5 * 60 * 1000 {
        return Err((StatusCode::BAD_REQUEST, "Timestamp too old.".into()));
    }

    let sig_content = format!("review_delete\n{}", review_id);
    if !verify_ed25519_signature(&q.key, &sig_content, q.timestamp, &q.sig) {
        return Err((StatusCode::UNAUTHORIZED, "Signature verification failed.".into()));
    }

    let user_role = state.db.get_role(&q.key).unwrap_or_default();
    let is_admin = user_role == "admin" || user_role == "mod";

    match state.db.delete_review(review_id, &q.key, is_admin) {
        Ok(true) => {
            let _ = state.broadcast_tx.send(crate::relay::RelayMessage::ReviewDeleted {
                listing_id,
                review_id,
            });
            Ok(Json(serde_json::json!({ "status": "deleted" })))
        }
        Ok(false) => Err((StatusCode::NOT_FOUND, "Review not found.".into())),
        Err(e) => Err((StatusCode::FORBIDDEN, e)),
    }
}

/// GET /api/sellers/{key}/rating — get aggregate seller rating.
pub async fn get_seller_rating(
    State(state): State<Arc<RelayState>>,
    axum::extract::Path(seller_key): axum::extract::Path<String>,
) -> Json<serde_json::Value> {
    let (avg, count) = state.db.get_seller_rating(&seller_key);
    Json(serde_json::json!({
        "seller_key": seller_key,
        "avg_rating": avg,
        "review_count": count,
    }))
}

// ── Order Book API ──

#[derive(Debug, Deserialize)]
pub struct OrderBookQuery {
    pub item_type: Option<String>,
}

/// GET /api/trade/orders?item_type=wood — get open sell orders for an item type.
pub async fn get_trade_orders(
    State(state): State<Arc<RelayState>>,
    Query(query): Query<OrderBookQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let item_type = query.item_type.unwrap_or_default();
    if item_type.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "item_type is required.".into()));
    }
    match state.db.get_open_orders(&item_type) {
        Ok(orders) => {
            let market_price = state.db.get_market_price(&item_type).unwrap_or(None);
            Ok(Json(serde_json::json!({
                "item_type": item_type,
                "orders": orders,
                "market_price": market_price,
            })))
        }
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}"))),
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateTradeOrderRequest {
    pub public_key: String,
    pub timestamp: u64,
    pub signature: String,
    pub item_type: String,
    pub item_id: Option<String>,
    pub quantity: i64,
    pub price_per_unit: f64,
    pub currency: Option<String>,
}

/// POST /api/trade/orders — create a sell order (authenticated via Ed25519 sig).
pub async fn create_trade_order(
    State(state): State<Arc<RelayState>>,
    Json(body): Json<CreateTradeOrderRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use crate::handlers::broadcast::verify_ed25519_signature;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    if now_ms.saturating_sub(body.timestamp) > 5 * 60 * 1000 {
        return Err((StatusCode::BAD_REQUEST, "Timestamp too old (> 5 min).".into()));
    }

    let sig_content = format!("trade_order\n{}\n{}\n{}", body.item_type, body.quantity, body.price_per_unit);
    if !verify_ed25519_signature(&body.public_key, &sig_content, body.timestamp, &body.signature) {
        return Err((StatusCode::UNAUTHORIZED, "Signature verification failed.".into()));
    }

    if body.quantity <= 0 {
        return Err((StatusCode::BAD_REQUEST, "Quantity must be positive.".into()));
    }
    if body.price_per_unit <= 0.0 {
        return Err((StatusCode::BAD_REQUEST, "Price must be positive.".into()));
    }
    if body.item_type.is_empty() || body.item_type.len() > 100 {
        return Err((StatusCode::BAD_REQUEST, "Invalid item_type.".into()));
    }

    let currency = body.currency.as_deref().unwrap_or("credits");
    if currency != "credits" && currency != "SOL" {
        return Err((StatusCode::BAD_REQUEST, "Currency must be 'credits' or 'SOL'.".into()));
    }

    let item_id = body.item_id.as_deref().unwrap_or("");
    match state.db.create_trade_order(
        &body.public_key,
        &body.item_type,
        item_id,
        body.quantity,
        body.price_per_unit,
        currency,
    ) {
        Ok(id) => Ok(Json(serde_json::json!({ "id": id, "status": "open" }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}"))),
    }
}

/// DELETE /api/trade/orders/{id} — cancel a sell order (authenticated via Ed25519 sig).
pub async fn cancel_trade_order(
    State(state): State<Arc<RelayState>>,
    axum::extract::Path(order_id): axum::extract::Path<i64>,
    Query(q): Query<VaultSyncQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use crate::handlers::broadcast::verify_ed25519_signature;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    if now_ms.saturating_sub(q.timestamp) > 5 * 60 * 1000 {
        return Err((StatusCode::BAD_REQUEST, "Timestamp too old.".into()));
    }

    let sig_content = format!("cancel_order\n{}", order_id);
    if !verify_ed25519_signature(&q.key, &sig_content, q.timestamp, &q.sig) {
        return Err((StatusCode::UNAUTHORIZED, "Signature verification failed.".into()));
    }

    match state.db.cancel_trade_order(order_id, &q.key) {
        Ok(true) => Ok(Json(serde_json::json!({ "status": "cancelled" }))),
        Ok(false) => Err((StatusCode::NOT_FOUND, "Order not found or not yours.".into())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}"))),
    }
}

#[derive(Debug, Deserialize)]
pub struct FillOrderRequest {
    pub public_key: String,
    pub timestamp: u64,
    pub signature: String,
    pub quantity: i64,
}

/// POST /api/trade/orders/{id}/fill — buy from a sell order (authenticated via Ed25519 sig).
pub async fn fill_trade_order(
    State(state): State<Arc<RelayState>>,
    axum::extract::Path(order_id): axum::extract::Path<i64>,
    Json(body): Json<FillOrderRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use crate::handlers::broadcast::verify_ed25519_signature;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    if now_ms.saturating_sub(body.timestamp) > 5 * 60 * 1000 {
        return Err((StatusCode::BAD_REQUEST, "Timestamp too old (> 5 min).".into()));
    }

    let sig_content = format!("fill_order\n{}\n{}", order_id, body.quantity);
    if !verify_ed25519_signature(&body.public_key, &sig_content, body.timestamp, &body.signature) {
        return Err((StatusCode::UNAUTHORIZED, "Signature verification failed.".into()));
    }

    match state.db.fill_trade_order(order_id, &body.public_key, body.quantity) {
        Ok(history_id) => Ok(Json(serde_json::json!({
            "status": "filled",
            "history_id": history_id,
        }))),
        Err(e) => Err((StatusCode::BAD_REQUEST, e)),
    }
}

#[derive(Debug, Deserialize)]
pub struct TradeHistoryQuery {
    pub key: Option<String>,
    pub limit: Option<usize>,
}

/// GET /api/trade/history?key=...&limit=50 — get trade history for a user.
pub async fn get_trade_history(
    State(state): State<Arc<RelayState>>,
    Query(query): Query<TradeHistoryQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let key = query.key.unwrap_or_default();
    if key.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "key is required.".into()));
    }
    let limit = query.limit.unwrap_or(50);
    match state.db.get_trade_history(&key, limit) {
        Ok(history) => Ok(Json(serde_json::json!(history))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}"))),
    }
}

/// GET /api/federation/servers — list federated servers (public).
pub async fn list_federation_servers(
    State(state): State<Arc<RelayState>>,
) -> Json<Vec<FederatedServerEntry>> {
    let servers = state.db.list_federated_servers().unwrap_or_default();
    let entries: Vec<FederatedServerEntry> = servers.into_iter().map(|s| FederatedServerEntry {
        server_id: s.server_id,
        name: s.name,
        url: s.url,
        public_key: s.public_key,
        trust_tier: s.trust_tier,
        accord_compliant: s.accord_compliant,
        status: s.status,
        last_seen: s.last_seen,
    }).collect();
    Json(entries)
}

/// Query parameters for GET /api/search.
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub channel: Option<String>,
    pub from: Option<String>,
    pub limit: Option<u32>,
}

/// GET /api/search?q=hello&channel=general&from=Michael&limit=20
pub async fn search_messages(
    State(state): State<Arc<RelayState>>,
    headers: HeaderMap,
    Query(params): Query<SearchQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_api_auth(&headers)?;

    if params.q.len() < 2 || params.q.len() > 200 {
        return Err((StatusCode::BAD_REQUEST, "Query must be 2-200 characters".into()));
    }

    let limit = params.limit.unwrap_or(50).min(100) as usize;
    // API search doesn't include DMs (no requester context); pass empty key to exclude DM results.
    match state.db.search_messages_full(&params.q, params.channel.as_deref(), params.from.as_deref(), limit, "") {
        Ok(results) => {
            let search_results: Vec<SearchResultData> = results.into_iter().map(|(id, ch, msg)| {
                if let RelayMessage::Chat { from, from_name, content, timestamp, .. } = msg {
                    SearchResultData {
                        message_id: id,
                        channel: ch,
                        from: from.clone(),
                        from_name: from_name.unwrap_or_default(),
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
            let total = search_results.len() as u32;
            Ok(Json(serde_json::json!({
                "query": params.q,
                "results": search_results,
                "total": total,
                "syntax": {
                    "description": "FTS5 full-text search",
                    "examples": {
                        "boolean": "hello AND world",
                        "phrase": "\"exact phrase\"",
                        "prefix": "hel*",
                        "exclude": "hello NOT goodbye",
                        "or": "hello OR hi",
                    }
                }
            })))
        }
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Search error: {e}"))),
    }
}

// ── Asset Library API ──

#[derive(Debug, Deserialize)]
pub struct AssetQuery {
    pub category: Option<String>,
    pub file_type: Option<String>,
    pub search: Option<String>,
    pub owner: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAssetRequest {
    pub filename: String,
    pub url: String,
    pub file_type: String,
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub size_bytes: u64,
    #[serde(default)]
    pub description: String,
    /// Public key of the owner (required).
    pub owner_key: String,
}

#[derive(Debug, Deserialize)]
pub struct AssetDeleteQuery {
    pub key: Option<String>,
    pub token: Option<String>,
}

/// GET /api/assets — list assets with optional filters.
pub async fn get_assets(
    State(state): State<Arc<RelayState>>,
    Query(query): Query<AssetQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let limit = query.limit.unwrap_or(50).min(200);
    let offset = query.offset.unwrap_or(0);
    // Fetch extra rows to support offset at the application layer.
    match state.db.get_assets(
        query.category.as_deref(),
        query.file_type.as_deref(),
        query.search.as_deref(),
        query.owner.as_deref(),
        limit + offset,
    ) {
        Ok(assets) => {
            let page: Vec<_> = assets.into_iter().skip(offset).collect();
            Ok(Json(serde_json::json!({ "assets": page })))
        }
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}"))),
    }
}

/// POST /api/assets — create asset metadata record after upload.
pub async fn create_asset(
    State(state): State<Arc<RelayState>>,
    Json(body): Json<CreateAssetRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Verify the owner is connected
    {
        let peers = state.peers.read().await;
        if !peers.contains_key(&body.owner_key) {
            return Err((StatusCode::FORBIDDEN, "Owner key is not connected.".into()));
        }
    }

    let id = format!("{}_{}", 
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis(),
        rand::rng().random::<u32>()
    );

    match state.db.create_asset(
        &id,
        &body.owner_key,
        &body.filename,
        &body.file_type,
        &body.category,
        &serde_json::to_string(&body.tags).unwrap_or_else(|_| "[]".to_string()),
        body.size_bytes as i64,
        &body.url,
        &body.description,
    ) {
        Ok(_) => Ok(Json(serde_json::json!({ "id": id, "status": "created" }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}"))),
    }
}

/// DELETE /api/assets/:id — delete an asset record.
pub async fn delete_asset(
    State(state): State<Arc<RelayState>>,
    axum::extract::Path(asset_id): axum::extract::Path<String>,
    Query(query): Query<AssetDeleteQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Resolve key from token or key param
    let public_key = if let Some(ref token) = query.token {
        let tokens = state.upload_tokens.read().await;
        tokens.get(token).cloned().ok_or_else(|| (StatusCode::FORBIDDEN, "Invalid token.".into()))?
    } else if let Some(ref k) = query.key {
        k.clone()
    } else {
        return Err((StatusCode::BAD_REQUEST, "Missing key or token.".into()));
    };

    let is_admin = {
        let role = state.db.get_role(&public_key).unwrap_or_default();
        role == "admin" || role == "mod"
    };

    match state.db.delete_asset(&asset_id, &public_key, is_admin) {
        Ok(true) => Ok(Json(serde_json::json!({ "status": "deleted" }))),
        Ok(false) => Err((StatusCode::NOT_FOUND, "Asset not found or not authorized.".into())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}"))),
    }
}

// ── Skill DNA API ──

#[derive(Debug, Deserialize)]
pub struct SkillSearchQuery {
    pub skill: String,
    pub min_level: Option<i32>,
    pub limit: Option<usize>,
}

pub async fn search_skills(
    state: axum::extract::State<Arc<RelayState>>,
    Query(query): Query<SkillSearchQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let min_level = query.min_level.unwrap_or(1);
    let limit = query.limit.unwrap_or(50).min(100);

    match state.db.search_skills(&query.skill, min_level, limit) {
        Ok(results) => {
            let users: Vec<serde_json::Value> = results.into_iter().map(|(key, rxp, fxp, lv)| {
                // Look up display name
                let name = state.db.get_display_name(&key).unwrap_or_default();
                serde_json::json!({
                    "public_key": key,
                    "display_name": name,
                    "reality_xp": rxp,
                    "fantasy_xp": fxp,
                    "level": lv,
                })
            }).collect();
            Ok(Json(serde_json::json!(users)))
        }
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Search error: {e}"))),
    }
}

#[derive(Debug, Deserialize)]
pub struct UserSkillQuery {
    pub key: Option<String>,
}

pub async fn get_user_skills(
    state: axum::extract::State<Arc<RelayState>>,
    axum::extract::Path(user_key): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    match state.db.get_user_skills(&user_key) {
        Ok(skills) => {
            let result: Vec<serde_json::Value> = skills.into_iter().map(|(sid, rxp, fxp, lv)| {
                serde_json::json!({
                    "skill_id": sid,
                    "reality_xp": rxp,
                    "fantasy_xp": fxp,
                    "level": lv,
                })
            }).collect();
            Ok(Json(serde_json::json!(result)))
        }
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}"))),
    }
}

// ── Vault Sync API ────────────────────────────────────────────────────────────
// Goal: let users back up their encrypted vault blob to the relay so they can
// restore it on another device. The blob is already AES-256-GCM encrypted by
// the client — we store it opaquely and never see plaintext.

#[derive(Debug, Deserialize)]
pub struct VaultSyncUpload {
    /// Ed25519 public key (hex) of the vault owner.
    pub key: String,
    /// Unix milliseconds when this request was made (anti-replay).
    pub timestamp: u64,
    /// sign("vault_sync\n" + timestamp, private_key) — proves ownership.
    pub sig: String,
    /// The encrypted vault blob (same format as localStorage hos_vault_v1).
    pub blob: String,
}

/// PUT /api/vault/sync — Upload (or replace) the encrypted vault blob.
pub async fn vault_sync_put(
    state: State<Arc<RelayState>>,
    Json(body): Json<VaultSyncUpload>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use crate::handlers::broadcast::verify_ed25519_signature;

    // Reject requests older than 5 minutes to prevent replay attacks.
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    if now_ms.saturating_sub(body.timestamp) > 5 * 60 * 1000 {
        return Err((StatusCode::BAD_REQUEST, "Timestamp too old (> 5 min).".into()));
    }

    if !verify_ed25519_signature(&body.key, "vault_sync", body.timestamp, &body.sig) {
        return Err((StatusCode::UNAUTHORIZED, "Signature verification failed.".into()));
    }

    // Cap blob size at 512 KB to prevent abuse.
    if body.blob.len() > 512 * 1024 {
        return Err((StatusCode::PAYLOAD_TOO_LARGE, "Vault blob exceeds 512 KB limit.".into()));
    }

    state.db.store_vault_blob(&body.key, &body.blob)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    Ok(Json(serde_json::json!({ "status": "stored", "updated_at": now_ms })))
}

#[derive(Debug, Deserialize)]
pub struct VaultSyncQuery {
    pub key: String,
    pub timestamp: u64,
    pub sig: String,
}

/// GET /api/vault/sync — Download the encrypted vault blob.
pub async fn vault_sync_get(
    state: State<Arc<RelayState>>,
    Query(q): Query<VaultSyncQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use crate::handlers::broadcast::verify_ed25519_signature;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    if now_ms.saturating_sub(q.timestamp) > 5 * 60 * 1000 {
        return Err((StatusCode::BAD_REQUEST, "Timestamp too old.".into()));
    }

    if !verify_ed25519_signature(&q.key, "vault_sync", q.timestamp, &q.sig) {
        return Err((StatusCode::UNAUTHORIZED, "Signature verification failed.".into()));
    }

    match state.db.get_vault_blob(&q.key) {
        Some((blob, updated_at)) => Ok(Json(serde_json::json!({ "blob": blob, "updated_at": updated_at }))),
        None => Err((StatusCode::NOT_FOUND, "No vault found for this key.".into())),
    }
}

/// DELETE /api/vault/sync — Wipe the server-side vault blob.
pub async fn vault_sync_delete(
    state: State<Arc<RelayState>>,
    Json(body): Json<VaultSyncUpload>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use crate::handlers::broadcast::verify_ed25519_signature;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    if now_ms.saturating_sub(body.timestamp) > 5 * 60 * 1000 {
        return Err((StatusCode::BAD_REQUEST, "Timestamp too old.".into()));
    }
    if !verify_ed25519_signature(&body.key, "vault_sync", body.timestamp, &body.sig) {
        return Err((StatusCode::UNAUTHORIZED, "Signature verification failed.".into()));
    }
    state.db.delete_vault_blob(&body.key)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

// ── System Profile API ───────────────────────────────────────────────────────
// Lets users store their system specs (OS, CPU, GPU, RAM, display) on the relay
// for cross-device access and AI context injection. Data is plain JSON — not
// sensitive, so no client-side encryption needed. Auth via Ed25519 signature.

#[derive(Debug, Deserialize)]
pub struct SystemProfileUpload {
    /// Ed25519 public key (hex) of the profile owner.
    pub key: String,
    /// Unix milliseconds when this request was made (anti-replay).
    pub timestamp: u64,
    /// sign("system_profile\n" + timestamp, private_key) — proves ownership.
    pub sig: String,
    /// System profile as a JSON string.
    pub profile: String,
}

#[derive(Debug, Deserialize)]
pub struct SystemProfileQuery {
    pub key: String,
    pub timestamp: u64,
    pub sig: String,
}

/// PUT /api/me/system — Store or replace the system profile.
pub async fn system_profile_put(
    state: State<Arc<RelayState>>,
    Json(body): Json<SystemProfileUpload>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use crate::handlers::broadcast::verify_ed25519_signature;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    if now_ms.saturating_sub(body.timestamp) > 5 * 60 * 1000 {
        return Err((StatusCode::BAD_REQUEST, "Timestamp too old (> 5 min).".into()));
    }

    if !verify_ed25519_signature(&body.key, "system_profile", body.timestamp, &body.sig) {
        return Err((StatusCode::UNAUTHORIZED, "Signature verification failed.".into()));
    }

    if body.profile.len() > 64 * 1024 {
        return Err((StatusCode::PAYLOAD_TOO_LARGE, "System profile exceeds 64 KB limit.".into()));
    }

    state.db.store_system_profile(&body.key, &body.profile)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    Ok(Json(serde_json::json!({ "status": "stored", "updated_at": now_ms })))
}

/// GET /api/me/system — Download the system profile.
pub async fn system_profile_get(
    state: State<Arc<RelayState>>,
    Query(q): Query<SystemProfileQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use crate::handlers::broadcast::verify_ed25519_signature;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    if now_ms.saturating_sub(q.timestamp) > 5 * 60 * 1000 {
        return Err((StatusCode::BAD_REQUEST, "Timestamp too old.".into()));
    }

    if !verify_ed25519_signature(&q.key, "system_profile", q.timestamp, &q.sig) {
        return Err((StatusCode::UNAUTHORIZED, "Signature verification failed.".into()));
    }

    match state.db.get_system_profile(&q.key) {
        Some((profile, updated_at)) => Ok(Json(serde_json::json!({ "profile": profile, "updated_at": updated_at }))),
        None => Err((StatusCode::NOT_FOUND, "No system profile found for this key.".into())),
    }
}

/// DELETE /api/me/system — Wipe the server-side system profile.
pub async fn system_profile_delete(
    state: State<Arc<RelayState>>,
    Json(body): Json<SystemProfileUpload>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use crate::handlers::broadcast::verify_ed25519_signature;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    if now_ms.saturating_sub(body.timestamp) > 5 * 60 * 1000 {
        return Err((StatusCode::BAD_REQUEST, "Timestamp too old.".into()));
    }
    if !verify_ed25519_signature(&body.key, "system_profile", body.timestamp, &body.sig) {
        return Err((StatusCode::UNAUTHORIZED, "Signature verification failed.".into()));
    }
    state.db.delete_system_profile(&body.key)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

// ── WebPush API ──────────────────────────────────────────────────────────────
// Goal: let browser clients register for push notifications so the relay can
// send alerts when the user receives a DM or @mention while offline.

/// GET /api/vapid-public-key — Returns the server's VAPID public key (base64url).
/// The key is returned in uncompressed SEC1 format (65 bytes, 0x04 prefix)
/// which is what the browser's PushManager.subscribe() expects.
pub async fn get_vapid_public_key(
    state: State<Arc<RelayState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use base64ct::{Base64UrlUnpadded, Encoding};
    use web_push_native::jwt_simple::prelude::ECDSAP256KeyPairLike;

    let kp = state.vapid_key.as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "VAPID not configured.".into()))?;
    // to_bytes() may return compressed (33 bytes) — browser needs uncompressed (65 bytes).
    // If compressed, decompress via p256. If already 65 bytes, use as-is.
    let pk_bytes = kp.public_key().to_bytes();
    let uncompressed = if pk_bytes.len() == 65 {
        pk_bytes
    } else {
        // Decompress via p256 crate.
        use web_push_native::p256::PublicKey;
        use web_push_native::p256::elliptic_curve::sec1::ToEncodedPoint;
        let pk = PublicKey::from_sec1_bytes(&pk_bytes)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Key decode error: {e}")))?;
        pk.to_encoded_point(false).as_bytes().to_vec()
    };
    let encoded = Base64UrlUnpadded::encode_string(&uncompressed);
    Ok(Json(serde_json::json!({ "key": encoded })))
}

#[derive(Debug, Deserialize)]
pub struct PushSubscribeRequest {
    /// Ed25519 public key (hex) of the subscribing user.
    pub public_key: String,
    /// Push service endpoint URL from PushSubscription.
    pub endpoint: String,
    /// P-256 DH public key from PushSubscription.keys.p256dh (base64url).
    pub p256dh: String,
    /// Auth secret from PushSubscription.keys.auth (base64url).
    pub auth: String,
    /// Unix milliseconds (anti-replay).
    pub timestamp: u64,
    /// sign("push_subscribe\n" + timestamp, private_key).
    pub sig: String,
}

/// POST /api/push/subscribe — Register a push subscription for a user.
pub async fn push_subscribe(
    state: State<Arc<RelayState>>,
    Json(body): Json<PushSubscribeRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use crate::handlers::broadcast::verify_ed25519_signature;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    if now_ms.saturating_sub(body.timestamp) > 5 * 60 * 1000 {
        return Err((StatusCode::BAD_REQUEST, "Timestamp too old.".into()));
    }
    if !verify_ed25519_signature(&body.public_key, "push_subscribe", body.timestamp, &body.sig) {
        return Err((StatusCode::UNAUTHORIZED, "Signature verification failed.".into()));
    }

    state.db.save_push_subscription(&body.public_key, &body.endpoint, &body.p256dh, &body.auth)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    tracing::info!("Push subscription registered for key {}", &body.public_key[..8.min(body.public_key.len())]);
    Ok(Json(serde_json::json!({ "status": "subscribed" })))
}

#[derive(Debug, Deserialize)]
pub struct PushUnsubscribeRequest {
    /// Push service endpoint URL to remove.
    pub endpoint: String,
}

/// POST /api/push/unsubscribe — Remove a push subscription.
pub async fn push_unsubscribe(
    state: State<Arc<RelayState>>,
    Json(body): Json<PushUnsubscribeRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    state.db.remove_push_subscription(&body.endpoint)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
    Ok(Json(serde_json::json!({ "status": "unsubscribed" })))
}

// ── Asset Manifest API ───────────────────────────────────────────────────────
// Scans the static file directories and returns every asset on disk so the
// dev page (and future game loader) can discover files dynamically instead of
// maintaining a hardcoded list.

/// Directory spec: (relative path from web root, category name, allowed extensions).
const ASSET_DIRS: &[(&str, &str, &[&str])] = &[
    ("assets/icons",             "icons",     &["png", "svg"]),
    ("assets/concepts",          "concepts",  &["png", "jpg", "jpeg", "webp"]),
    ("web/shared/icons",          "app-icons", &["png", "svg", "ico"]),
    ("desktop/src-tauri/icons",  "desktop-icons", &["png", "ico", "icns", "svg"]),
    ("assets/textures",          "textures",  &["ktx2", "png", "jpg", "jpeg", "webp"]),
    ("assets/models",            "models",    &["glb", "gltf"]),
    ("assets/audio",             "audio",     &["ogg", "flac", "mp3", "wav"]),
    ("assets/shaders",           "shaders",   &["wgsl", "glsl", "frag", "vert"]),
    ("assets/fonts",             "fonts",     &["ttf", "woff2", "woff", "otf"]),
];

#[derive(Serialize)]
struct AssetEntry {
    path: String,
    filename: String,
    extension: String,
    category: String,
    size_bytes: u64,
    modified: u64,
}

/// GET /api/asset-manifest — Scan static directories and return every asset on disk.
pub async fn get_asset_manifest() -> Json<serde_json::Value> {
    let web_root = std::env::var("WEB_ROOT").unwrap_or_else(|_| ".".to_string());
    let base = std::path::Path::new(&web_root);

    let mut assets: Vec<AssetEntry> = Vec::new();
    let mut categories: std::collections::HashMap<String, u64> = std::collections::HashMap::new();

    for &(dir_rel, category, extensions) in ASSET_DIRS {
        let dir_path = base.join(dir_rel);
        let entries = match std::fs::read_dir(&dir_path) {
            Ok(e) => e,
            Err(_) => continue, // directory may not exist yet
        };

        for entry in entries.flatten() {
            let file_path = entry.path();
            if !file_path.is_file() {
                continue;
            }

            let file_name = match file_path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };

            let ext = file_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            // Skip files whose extension is not in the allow-list for this dir.
            if !extensions.iter().any(|&allowed| allowed == ext) {
                continue;
            }

            let meta = match std::fs::metadata(&file_path) {
                Ok(m) => m,
                Err(_) => continue,
            };

            let size_bytes = meta.len();
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            // Build URL path with forward slashes regardless of OS.
            let url_path = format!("/{}", dir_rel.replace('\\', "/")) + "/" + &file_name;

            assets.push(AssetEntry {
                path: url_path,
                filename: file_name,
                extension: ext,
                category: category.to_string(),
                size_bytes,
                modified,
            });

            *categories.entry(category.to_string()).or_insert(0) += 1;
        }
    }

    // Sort by category then filename for deterministic output.
    assets.sort_by(|a, b| a.category.cmp(&b.category).then(a.filename.cmp(&b.filename)));

    let total = assets.len();
    Json(serde_json::json!({
        "assets": assets,
        "categories": categories,
        "total": total,
    }))
}

// ── Web Manifest API ────────────────────────────────────────────────────────
// Returns a hashed manifest of all web-servable files so the desktop app can
// compare local vs remote and sync only changed files.

use std::sync::Mutex;

/// Directories to include in the web manifest, relative to WEB_ROOT.
/// Each entry: (dir path, allowed extensions, recursive).
const WEB_MANIFEST_DIRS: &[(&str, &[&str], bool)] = &[
    ("web/shared",                     &["js", "css", "json"],           false),
    ("web/shared/icons",               &["png", "svg", "ico"],           false),
    ("web/pages",                      &["html", "js", "css"],           false),
    ("web/activities",                  &["html", "js", "css"],           false),
    ("web/chat",                       &["html", "js", "css", "ico", "png", "svg"], false),
    ("assets/icons",                  &["png", "svg"],                  false),
];

#[derive(Serialize, Clone)]
pub struct WebManifestFile {
    pub path: String,
    pub hash: String,
    pub size: u64,
    pub modified: u64,
}

#[derive(Serialize, Clone)]
pub struct WebManifest {
    pub version: String,
    pub files: Vec<WebManifestFile>,
    pub total_size: u64,
    pub file_count: usize,
}

/// Cached web manifest with expiry timestamp.
struct CachedWebManifest {
    manifest: WebManifest,
    expires_at: std::time::Instant,
}

static WEB_MANIFEST_CACHE: std::sync::LazyLock<Mutex<Option<CachedWebManifest>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

/// Scan directories and build the web manifest.
fn build_web_manifest() -> WebManifest {
    let web_root = std::env::var("WEB_ROOT").unwrap_or_else(|_| ".".to_string());
    let base = std::path::Path::new(&web_root);

    let mut files: Vec<WebManifestFile> = Vec::new();
    let mut total_size: u64 = 0;

    for &(dir_rel, extensions, _recursive) in WEB_MANIFEST_DIRS {
        let dir_path = base.join(dir_rel);
        let entries = match std::fs::read_dir(&dir_path) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let file_path = entry.path();
            if !file_path.is_file() {
                continue;
            }

            let file_name = match file_path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };

            let ext = file_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            if !extensions.iter().any(|&allowed| allowed == ext) {
                continue;
            }

            let meta = match std::fs::metadata(&file_path) {
                Ok(m) => m,
                Err(_) => continue,
            };

            let size = meta.len();
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            // Compute SHA-256 hash of file contents.
            let hash = match std::fs::read(&file_path) {
                Ok(contents) => {
                    let mut hasher = Sha256::new();
                    hasher.update(&contents);
                    format!("sha256:{}", hex::encode(hasher.finalize()))
                }
                Err(_) => continue,
            };

            // Map ui/chat/ → /client/ for URL paths (backward compat).
            let url_dir = if dir_rel == "web/chat" {
                "client".to_string()
            } else {
                dir_rel.replace('\\', "/")
            };
            let url_path = format!("/{}/{}", url_dir, file_name);

            total_size += size;
            files.push(WebManifestFile {
                path: url_path,
                hash,
                size,
                modified,
            });
        }
    }

    // Sort by path for deterministic output.
    files.sort_by(|a, b| a.path.cmp(&b.path));

    let file_count = files.len();

    // Read version from tauri.conf.json or fall back.
    let version = std::fs::read_to_string(base.join("desktop/src-tauri/tauri.conf.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("version")?.as_str().map(String::from))
        .unwrap_or_else(|| "0.0.0".to_string());

    WebManifest {
        version,
        files,
        total_size,
        file_count,
    }
}

/// GET /api/web-manifest — Hashed manifest of all web files for desktop sync.
pub async fn get_web_manifest() -> Json<WebManifest> {
    // Return cached manifest if still valid (60s TTL).
    {
        let cache = WEB_MANIFEST_CACHE.lock().unwrap();
        if let Some(ref cached) = *cache {
            if cached.expires_at > std::time::Instant::now() {
                return Json(cached.manifest.clone());
            }
        }
    }

    let manifest = build_web_manifest();

    // Store in cache.
    {
        let mut cache = WEB_MANIFEST_CACHE.lock().unwrap();
        *cache = Some(CachedWebManifest {
            manifest: manifest.clone(),
            expires_at: std::time::Instant::now() + std::time::Duration::from_secs(60),
        });
    }

    Json(manifest)
}

// ── Signed Profile Lookup ──

/// GET /api/profile/{key} — Look up a signed profile by public key.
/// Returns the cached signed profile if available. Any server that has
/// seen the user can serve their profile — no home server needed.
pub async fn get_signed_profile(
    State(state): State<Arc<RelayState>>,
    Path(key): Path<String>,
) -> impl IntoResponse {
    match state.db.get_signed_profile(&key) {
        Ok(Some(profile)) => {
            let body = serde_json::json!({
                "public_key": profile.public_key,
                "name": profile.name,
                "bio": profile.bio,
                "avatar_url": profile.avatar_url,
                "banner_url": profile.banner_url,
                "socials": serde_json::from_str::<serde_json::Value>(&profile.socials).unwrap_or_default(),
                "pronouns": profile.pronouns,
                "location": profile.location,
                "website": profile.website,
                "timestamp": profile.timestamp,
                "signature": profile.signature,
            });
            (StatusCode::OK, Json(body)).into_response()
        }
        Ok(None) => {
            (StatusCode::NOT_FOUND, Json(serde_json::json!({
                "error": "Profile not found"
            }))).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": format!("Database error: {}", e)
            }))).into_response()
        }
    }
}

// ── File browser API ───────────────────────────────────────────────────

/// Query params for GET /api/files.
#[derive(Debug, Deserialize)]
pub struct FilesListQuery {
    /// Directory path to list (must be within data/).
    pub path: Option<String>,
}

/// GET /api/files — list files in a directory within data/.
pub async fn list_files(
    Query(params): Query<FilesListQuery>,
) -> impl IntoResponse {
    let dir_path = params.path.as_deref().unwrap_or("data");

    match crate::storage::files::list_directory(dir_path) {
        Ok(entries) => (StatusCode::OK, Json(serde_json::json!({
            "path": dir_path,
            "entries": entries,
        }))).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": e,
        }))).into_response(),
    }
}

/// Query params for GET /api/files/read.
#[derive(Debug, Deserialize)]
pub struct FileReadQuery {
    /// File path to read (must be within data/).
    pub path: String,
}

/// GET /api/files/read — read a text file's contents.
pub async fn read_file(
    Query(params): Query<FileReadQuery>,
) -> impl IntoResponse {
    match crate::storage::files::read_file(&params.path) {
        Ok(content) => {
            let abs_path = crate::storage::files::validate_path(&params.path);
            let (size, modified) = abs_path.ok()
                .and_then(|p| std::fs::metadata(&p).ok())
                .map(|m| {
                    let size = m.len();
                    let modified = m.modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    (size, modified)
                })
                .unwrap_or((0, 0));

            (StatusCode::OK, Json(serde_json::json!({
                "path": params.path,
                "content": content,
                "size": size,
                "modified": modified,
            }))).into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": e,
        }))).into_response(),
    }
}

/// Request body for POST /api/files/write.
#[derive(Debug, Deserialize)]
pub struct FileWriteRequest {
    /// File path to write (must be within data/).
    pub path: String,
    /// File content to write.
    pub content: String,
}

/// POST /api/files/write — write/save a text file.
pub async fn write_file(
    Json(req): Json<FileWriteRequest>,
) -> impl IntoResponse {
    match crate::storage::files::write_file(&req.path, &req.content) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({
            "ok": true,
            "path": req.path,
        }))).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": e,
        }))).into_response(),
    }
}

// ── Admin Analytics API ──

/// Query params for GET /api/admin/stats (Ed25519-signed).
#[derive(Debug, Deserialize)]
pub struct AdminStatsQuery {
    pub key: String,
    pub timestamp: u64,
    pub sig: String,
}

/// GET /api/admin/stats — admin-only server analytics.
/// Authenticated via Ed25519 signature (same pattern as vault_sync).
pub async fn get_admin_stats(
    State(state): State<Arc<RelayState>>,
    Query(q): Query<AdminStatsQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use crate::handlers::broadcast::verify_ed25519_signature;

    // Verify freshness (5 min window).
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    if now_ms.saturating_sub(q.timestamp) > 5 * 60 * 1000 {
        return Err((StatusCode::BAD_REQUEST, "Timestamp too old.".into()));
    }

    // Verify Ed25519 signature.
    if !verify_ed25519_signature(&q.key, "admin_stats", q.timestamp, &q.sig) {
        return Err((StatusCode::UNAUTHORIZED, "Signature verification failed.".into()));
    }

    // Check admin role.
    let role = state.db.get_role(&q.key).unwrap_or_default();
    if role != "admin" {
        return Err((StatusCode::FORBIDDEN, "Admin role required.".into()));
    }

    // Gather stats.
    let user_count = state.db.get_member_count(None).unwrap_or(0);
    let online_count = state.peers.read().await.len();
    let total_messages = state.db.message_count().unwrap_or(0);
    let uptime_seconds = state.start_time.elapsed().as_secs();

    // Message count in last 24h.
    let message_count_24h = state.db.message_count_since_hours(24).unwrap_or(0);

    // DB file size.
    let db_size_bytes = std::fs::metadata("data/relay.db")
        .map(|m| m.len())
        .unwrap_or(0);

    // Upload storage size.
    let upload_size_bytes = std::fs::read_dir("data/uploads")
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter_map(|e| e.metadata().ok())
                .filter(|m| m.is_file())
                .map(|m| m.len())
                .sum::<u64>()
        })
        .unwrap_or(0);

    // Top channels by message count.
    let top_channels = state.db.top_channels_by_messages(10).unwrap_or_default();

    // Recent joins (last 10).
    let recent_joins = state.db.recent_joins(10).unwrap_or_default();

    // Federation servers.
    let federation_servers = state.db.list_federated_servers().unwrap_or_default();
    let federation: Vec<serde_json::Value> = federation_servers.into_iter().map(|s| {
        serde_json::json!({
            "server_id": s.server_id,
            "name": s.name,
            "url": s.url,
            "trust_tier": s.trust_tier,
            "status": s.status,
            "last_seen": s.last_seen,
        })
    }).collect();

    // Messages per hour (last 24h) for activity chart.
    let hourly_messages = state.db.messages_per_hour_24h().unwrap_or_default();

    // Game world stats.
    let game_world = state.game_world.read().await;
    let game_players = game_world.player_count();
    let game_entities = game_world.entity_count();
    let game_time = game_world.game_time;
    drop(game_world);

    Ok(Json(serde_json::json!({
        "user_count": user_count,
        "online_count": online_count,
        "total_messages": total_messages,
        "message_count_24h": message_count_24h,
        "db_size_bytes": db_size_bytes,
        "upload_size_bytes": upload_size_bytes,
        "uptime_seconds": uptime_seconds,
        "top_channels": top_channels,
        "recent_joins": recent_joins,
        "federation": federation,
        "hourly_messages": hourly_messages,
        "game_players": game_players,
        "game_entities": game_entities,
        "game_time": game_time,
    })))
}

// ── Guilds API ──────────────────────────────────────────────────────────

/// Query params for GET /api/guilds.
#[derive(Debug, Deserialize)]
pub struct GuildsQuery {
    pub search: Option<String>,
    pub user: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// GET /api/guilds — list or search guilds.
pub async fn get_guilds(
    State(state): State<Arc<RelayState>>,
    Query(params): Query<GuildsQuery>,
) -> Json<serde_json::Value> {
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);

    let guilds = if let Some(ref user) = params.user {
        state.db.get_guilds_for_user(user).unwrap_or_default()
    } else if let Some(ref q) = params.search {
        state.db.search_guilds(q, limit, offset).unwrap_or_default()
    } else {
        state.db.get_all_guilds(limit, offset).unwrap_or_default()
    };

    let list: Vec<serde_json::Value> = guilds.into_iter().map(|g| {
        serde_json::json!({
            "id": g.id,
            "name": g.name,
            "description": g.description,
            "owner_key": g.owner_key,
            "icon": g.icon,
            "color": g.color,
            "created_at": g.created_at,
            "member_count": g.member_count,
        })
    }).collect();
    Json(serde_json::json!(list))
}

/// Request body for POST /api/guilds.
#[derive(Debug, Deserialize)]
pub struct CreateGuildRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default = "default_guild_color")]
    pub color: String,
    pub owner_key: String,
}

fn default_guild_color() -> String { "#4488ff".to_string() }

/// POST /api/guilds — create a new guild.
pub async fn create_guild(
    State(state): State<Arc<RelayState>>,
    Json(req): Json<CreateGuildRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if req.name.trim().is_empty() || req.name.len() > 100 {
        return Err((StatusCode::BAD_REQUEST, "Guild name must be 1-100 characters.".into()));
    }
    if req.description.len() > 2000 {
        return Err((StatusCode::BAD_REQUEST, "Description too long (max 2000 chars).".into()));
    }
    if req.owner_key.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Owner key is required.".into()));
    }

    let id = uuid::Uuid::new_v4().to_string();
    match state.db.create_guild(&id, &req.name, &req.description, &req.owner_key, &req.icon, &req.color) {
        Ok(()) => Ok(Json(serde_json::json!({ "id": id, "status": "created" }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create guild: {e}"))),
    }
}

/// GET /api/guilds/{id} — get a single guild.
pub async fn get_guild(
    State(state): State<Arc<RelayState>>,
    Path(guild_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let guild = state.db.get_guild(&guild_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(serde_json::json!({
        "id": guild.id,
        "name": guild.name,
        "description": guild.description,
        "owner_key": guild.owner_key,
        "icon": guild.icon,
        "color": guild.color,
        "created_at": guild.created_at,
        "member_count": guild.member_count,
    })))
}

/// Request body for PATCH /api/guilds/{id}.
#[derive(Debug, Deserialize)]
pub struct UpdateGuildRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub owner_key: String,
}

/// PATCH /api/guilds/{id} — update a guild.
pub async fn update_guild(
    State(state): State<Arc<RelayState>>,
    Path(guild_id): Path<String>,
    Json(req): Json<UpdateGuildRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let existing = state.db.get_guild(&guild_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Guild not found.".into()))?;

    let name = req.name.as_deref().unwrap_or(&existing.name);
    let description = req.description.as_deref().unwrap_or(&existing.description);
    let icon = req.icon.as_deref().unwrap_or(&existing.icon);
    let color = req.color.as_deref().unwrap_or(&existing.color);

    if name.is_empty() || name.len() > 100 {
        return Err((StatusCode::BAD_REQUEST, "Guild name must be 1-100 characters.".into()));
    }

    match state.db.update_guild(&guild_id, &req.owner_key, name, description, icon, color) {
        Ok(true) => Ok(Json(serde_json::json!({ "status": "updated" }))),
        Ok(false) => Err((StatusCode::FORBIDDEN, "Only the guild owner can update.".into())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to update guild: {e}"))),
    }
}

/// DELETE /api/guilds/{id} — delete a guild.
pub async fn delete_guild(
    State(state): State<Arc<RelayState>>,
    Path(guild_id): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let owner_key = params.get("owner_key")
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "owner_key query param required.".into()))?;

    match state.db.delete_guild(&guild_id, owner_key) {
        Ok(true) => Ok(Json(serde_json::json!({ "status": "deleted", "id": guild_id }))),
        Ok(false) => Err((StatusCode::FORBIDDEN, "Only the guild owner can delete, or guild not found.".into())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to delete guild: {e}"))),
    }
}

/// Query params for GET /api/guilds/{id}/members.
#[derive(Debug, Deserialize)]
pub struct GuildMembersQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// GET /api/guilds/{id}/members — list guild members.
pub async fn get_guild_members(
    State(state): State<Arc<RelayState>>,
    Path(guild_id): Path<String>,
    Query(params): Query<GuildMembersQuery>,
) -> Json<serde_json::Value> {
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);
    let members = state.db.get_guild_members(&guild_id, limit, offset).unwrap_or_default();
    let list: Vec<serde_json::Value> = members.into_iter().map(|m| {
        serde_json::json!({
            "guild_id": m.guild_id,
            "public_key": m.public_key,
            "role": m.role,
            "joined_at": m.joined_at,
            "name": m.name,
        })
    }).collect();
    Json(serde_json::json!(list))
}

/// Request body for POST /api/guilds/{id}/members (join).
#[derive(Debug, Deserialize)]
pub struct JoinGuildRequest {
    pub public_key: String,
    pub invite_code: Option<String>,
}

/// POST /api/guilds/{id}/members — join a guild (directly or via invite code).
pub async fn join_guild(
    State(state): State<Arc<RelayState>>,
    Path(guild_id): Path<String>,
    Json(req): Json<JoinGuildRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if let Some(code) = &req.invite_code {
        match state.db.use_guild_invite(code, &req.public_key) {
            Ok(Some(gid)) => Ok(Json(serde_json::json!({ "status": "joined", "guild_id": gid }))),
            Ok(None) => Err((StatusCode::BAD_REQUEST, "Invalid or expired invite code.".into())),
            Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to use invite: {e}"))),
        }
    } else {
        match state.db.join_guild(&guild_id, &req.public_key) {
            Ok(true) => Ok(Json(serde_json::json!({ "status": "joined" }))),
            Ok(false) => Err((StatusCode::CONFLICT, "Already a member.".into())),
            Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to join guild: {e}"))),
        }
    }
}

/// Request body for POST /api/guilds/{id}/leave.
#[derive(Debug, Deserialize)]
pub struct LeaveGuildRequest {
    pub public_key: String,
}

/// POST /api/guilds/{id}/leave — leave a guild.
pub async fn leave_guild(
    State(state): State<Arc<RelayState>>,
    Path(guild_id): Path<String>,
    Json(req): Json<LeaveGuildRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    match state.db.leave_guild(&guild_id, &req.public_key) {
        Ok(true) => Ok(Json(serde_json::json!({ "status": "left" }))),
        Ok(false) => Err((StatusCode::BAD_REQUEST, "Cannot leave: you are the owner or not a member.".into())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to leave guild: {e}"))),
    }
}

/// Request body for POST /api/guilds/{id}/invite.
#[derive(Debug, Deserialize)]
pub struct CreateGuildInviteRequest {
    pub created_by: String,
    #[serde(default = "default_invite_uses")]
    pub uses: i64,
    /// Duration in hours before expiry (default: 24).
    #[serde(default = "default_invite_hours")]
    pub hours: i64,
}

fn default_invite_uses() -> i64 { 10 }
fn default_invite_hours() -> i64 { 24 }

/// POST /api/guilds/{id}/invite — create an invite code.
pub async fn create_guild_invite(
    State(state): State<Arc<RelayState>>,
    Path(guild_id): Path<String>,
    Json(req): Json<CreateGuildInviteRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Only members can create invites.
    let role = state.db.get_guild_member_role(&guild_id, &req.created_by);
    if role.is_none() {
        return Err((StatusCode::FORBIDDEN, "Only guild members can create invites.".into()));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let code: String = {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..8).map(|_| {
            let idx = rng.gen_range(0..36);
            if idx < 10 { (b'0' + idx) as char }
            else { (b'a' + idx - 10) as char }
        }).collect()
    };

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let expires_at = now_ms + req.hours * 3600 * 1000;

    match state.db.create_guild_invite(&id, &guild_id, &req.created_by, &code, req.uses, expires_at) {
        Ok(()) => Ok(Json(serde_json::json!({
            "id": id,
            "code": code,
            "uses_remaining": req.uses,
            "expires_at": expires_at,
        }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create invite: {e}"))),
    }
}

// ── Reputation API ──────────────────────────────────────────────────────

/// GET /api/reputation/{key} — get a user's reputation.
pub async fn get_reputation(
    State(state): State<Arc<RelayState>>,
    Path(key): Path<String>,
) -> impl IntoResponse {
    match state.db.get_reputation(&key) {
        Ok(rep) => {
            let history = state.db.get_reputation_history(&key, 20).unwrap_or_default();
            let events: Vec<serde_json::Value> = history.into_iter().map(|e| {
                serde_json::json!({
                    "id": e.id,
                    "event_type": e.event_type,
                    "points": e.points,
                    "reason": e.reason,
                    "created_at": e.created_at,
                    "source_key": e.source_key,
                })
            }).collect();
            (StatusCode::OK, Json(serde_json::json!({
                "public_key": rep.public_key,
                "score": rep.score,
                "level": rep.level,
                "updated_at": rep.updated_at,
                "recent_events": events,
            }))).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": format!("Database error: {}", e)
            }))).into_response()
        }
    }
}

/// Query params for GET /api/reputation/leaderboard.
#[derive(Debug, Deserialize)]
pub struct LeaderboardQuery {
    pub limit: Option<usize>,
}

/// GET /api/reputation/leaderboard — top users by reputation score.
pub async fn get_reputation_leaderboard(
    State(state): State<Arc<RelayState>>,
    Query(params): Query<LeaderboardQuery>,
) -> Json<serde_json::Value> {
    let limit = params.limit.unwrap_or(20).min(100);
    let leaders = state.db.get_reputation_leaderboard(limit).unwrap_or_default();
    let list: Vec<serde_json::Value> = leaders.into_iter().map(|r| {
        // Try to get display name from server_members.
        let name = state.db.get_member(&r.public_key)
            .ok()
            .flatten()
            .and_then(|m| m.name);
        serde_json::json!({
            "public_key": r.public_key,
            "score": r.score,
            "level": r.level,
            "name": name,
        })
    }).collect();
    Json(serde_json::json!(list))
}
