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

use rand::Rng;
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
    const MAX_SIZE_DEFAULT: usize = 5 * 1024 * 1024; // 5MB for most files
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
    let webhook_secret = std::env::var("WEBHOOK_SECRET").unwrap_or_default();
    if webhook_secret.is_empty() {
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "WEBHOOK_SECRET not configured.".into()));
    }

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
}

fn default_backlog() -> String { "backlog".to_string() }
fn default_medium() -> String { "medium".to_string() }
fn default_empty_labels() -> String { "[]".to_string() }

/// GET /api/tasks — list all project board tasks.
pub async fn get_tasks(
    State(state): State<Arc<RelayState>>,
    Query(params): Query<TasksQuery>,
) -> Json<TasksResponse> {
    let tasks = state.db.list_tasks().unwrap_or_default();
    let counts = state.db.get_task_comment_counts().unwrap_or_default();
    let entries: Vec<TaskEntry> = tasks.into_iter()
        .filter(|t| params.status.as_ref().map_or(true, |s| &t.status == s))
        .map(|t| {
            let cc = *counts.get(&t.id).unwrap_or(&0);
            TaskEntry {
                id: t.id, title: t.title, description: t.description,
                status: t.status, priority: t.priority, assignee: t.assignee,
                created_by: t.created_by, created_at: t.created_at,
                updated_at: t.updated_at, labels: t.labels, comment_count: cc,
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

    match state.db.create_task(&req.title, &req.description, status, priority, req.assignee.as_deref(), "bot_api", &req.labels) {
        Ok(id) => {
            // Broadcast to WebSocket clients.
            if let Ok(Some(task)) = state.db.get_task(id) {
                let td = crate::relay::TaskData {
                    id: task.id, title: task.title, description: task.description,
                    status: task.status, priority: task.priority, assignee: task.assignee,
                    created_by: task.created_by, created_at: task.created_at,
                    updated_at: task.updated_at, position: task.position, labels: task.labels,
                    comment_count: 0,
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

    if title.is_empty() || title.len() > 200 {
        return Err((StatusCode::BAD_REQUEST, "Title must be 1-200 characters.".into()));
    }

    let valid_priorities = ["low", "medium", "high", "critical"];
    if !valid_priorities.contains(&priority) {
        return Err((StatusCode::BAD_REQUEST, format!("Invalid priority '{}'.", priority)));
    }

    // Only call update if non-status fields changed.
    let fields_changed = req.title.is_some() || req.description.is_some() || req.priority.is_some() || req.assignee.is_some() || req.labels.is_some();
    if fields_changed {
        state.db.update_task(task_id, title, description, priority, assignee, labels)
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
        comment_count: cc,
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

// ── Federation API ──

/// Response for GET /api/server-info.
#[derive(Debug, Serialize)]
pub struct ServerInfoResponse {
    pub server_id: String,
    pub name: String,
    pub version: &'static str,
    pub channels: Vec<String>,
    pub users_online: usize,
    pub accord_compliant: bool,
    pub public_key: String,
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
    let server_name = std::env::var("SERVER_NAME").unwrap_or_else(|_| "Humanity Relay".to_string());
    let accord = std::env::var("ACCORD_COMPLIANT").unwrap_or_default() == "true";

    Json(ServerInfoResponse {
        server_id: pk.clone(),
        name: server_name,
        version: env!("BUILD_VERSION"),
        channels,
        users_online,
        accord_compliant: accord,
        public_key: pk,
    })
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
    pub status: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// GET /api/listings — browse marketplace listings (public).
pub async fn get_listings(
    State(state): State<Arc<RelayState>>,
    Query(params): Query<ListingsQuery>,
) -> Json<ListingsResponse> {
    let limit = params.limit.unwrap_or(50).min(200);
    let listings = state.db.get_listings(
        params.category.as_deref(),
        params.status.as_deref().or(Some("active")),
        limit,
    ).unwrap_or_default();
    let entries: Vec<ListingEntry> = listings.into_iter().map(|l| ListingEntry {
        id: l.id, seller_key: l.seller_key, seller_name: l.seller_name,
        title: l.title, description: l.description, category: l.category,
        condition: l.condition, price: l.price, payment_methods: l.payment_methods,
        location: l.location, status: l.status, created_at: l.created_at,
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
    let limit = query.limit.unwrap_or(100).min(500);
    match state.db.get_assets(
        query.category.as_deref(),
        query.file_type.as_deref(),
        query.search.as_deref(),
        query.owner.as_deref(),
        limit,
    ) {
        Ok(assets) => Ok(Json(serde_json::json!({ "assets": assets }))),
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
