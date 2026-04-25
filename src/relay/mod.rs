//! Humanity Network relay server.
//!
//! A WebSocket relay that routes signed objects between connected clients.
//! This is the mandatory fallback transport defined in the hybrid network
//! architecture (design/architecture_decisions/hybrid_network.md).

pub mod relay;
pub mod api;
pub mod api_v2_ai;
pub mod api_v2_credentials;
pub mod api_v2_did;
pub mod api_v2_governance;
pub mod api_v2_objects;
pub mod api_v2_recovery;
pub mod api_v2_trust;
pub mod storage;
pub mod handlers;
pub mod core;

use axum::{
    Router,
    routing::{get, post, delete, patch},
    extract::ws::{WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    middleware,
};
use axum::http::{self, HeaderMap, HeaderValue, StatusCode};
use tower_http::cors::CorsLayer;
use std::sync::Arc;

use relay::RelayState;
use serde_json;
use rusqlite;

/// Add security headers to every response.
/// CSP uses unsafe-inline for now (inline scripts/handlers exist throughout the
/// client); this still blocks external script injection and eval().
/// X-Frame-Options + CSP frame-ancestors together prevent clickjacking.
async fn security_headers(
    req: axum::extract::Request,
    next: middleware::Next,
) -> axum::response::Response {
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert("x-content-type-options",   HeaderValue::from_static("nosniff"));
    h.insert("x-frame-options",          HeaderValue::from_static("SAMEORIGIN"));
    h.insert("referrer-policy",          HeaderValue::from_static("strict-origin-when-cross-origin"));
    h.insert("permissions-policy",       HeaderValue::from_static("camera=(), microphone=(), geolocation=(), payment=()"));
    h.insert("content-security-policy",  HeaderValue::from_static(
        "default-src 'self'; \
         script-src 'self' 'unsafe-inline'; \
         style-src 'self' 'unsafe-inline'; \
         img-src 'self' data: https:; \
         connect-src 'self' wss://united-humanity.us wss://chat.united-humanity.us; \
         font-src 'self'; \
         frame-src 'self'; \
         object-src 'none'; \
         base-uri 'self'; \
         form-action 'self';"
    ));
    res
}

/// Convert epoch days (since 1970-01-01) to (year, month, day).
fn epoch_days_to_ymd(days: i64) -> (i64, u32, u32) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Validate environment variables and configuration at startup.
/// Fails fast with helpful messages if critical config is invalid.
pub fn validate_environment() -> (String, u16) {
    let db_path = std::env::var("DATABASE_PATH")
        .or_else(|_| std::env::var("DB_PATH"))
        .unwrap_or_else(|_| {
            tracing::info!("DATABASE_PATH not set, defaulting to data/relay.db");
            "data/relay.db".to_string()
        });

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or_else(|| {
            tracing::info!("PORT not set, defaulting to 3210");
            3210
        });

    // Validate database directory exists or can be created.
    let db_dir = std::path::Path::new(&db_path)
        .parent()
        .unwrap_or(std::path::Path::new("."));
    if let Err(e) = std::fs::create_dir_all(db_dir) {
        panic!("Cannot create database directory '{}': {e}. Set DATABASE_PATH to a writable location.", db_dir.display());
    }

    // Warn about optional but recommended config.
    if std::env::var("SERVER_NAME").is_err() {
        tracing::warn!("SERVER_NAME not set -- using default server identity");
    }
    if std::env::var("VAPID_PRIVATE_KEY").is_err() {
        tracing::warn!("VAPID keys not configured -- web push notifications will be disabled");
    }
    if std::env::var("API_SECRET").is_err() {
        tracing::warn!("API_SECRET not set -- bot API endpoints will reject all requests");
    }
    if std::env::var("WEBHOOK_SECRET").is_err() {
        tracing::warn!("WEBHOOK_SECRET not set -- GitHub webhook endpoint will reject requests");
    }

    tracing::info!("Configuration validated: db={db_path}, port={port}");
    (db_path, port)
}

/// Run the relay server. Contains all startup logic from main().
/// Call after initializing logging and validating the environment.
pub async fn run_relay() {
    // Validate environment and get config.
    let (db_path, port) = validate_environment();

    // Initialize persistent storage.
    let db_dir = std::path::Path::new(&db_path).parent().unwrap_or(std::path::Path::new("."));
    std::fs::create_dir_all(db_dir).expect("Failed to create database directory");

    let db = storage::Storage::open(std::path::Path::new(&db_path))
        .expect("Failed to open database");

    let msg_count = db.message_count().unwrap_or(0);
    tracing::info!("Database has {msg_count} stored messages");

    // Auto-promote first registered user or ADMIN_KEYS to admin.
    if let Ok(admin_keys) = std::env::var("ADMIN_KEYS") {
        for key in admin_keys.split(',') {
            let key = key.trim();
            if !key.is_empty() {
                if let Err(e) = db.set_role(key, "admin") {
                    tracing::error!("Failed to set admin for {key}: {e}");
                } else {
                    tracing::info!("Admin role set for key: {key}");
                }
            }
        }
    }

    // Generate or load server Ed25519 keypair for federation.
    match db.get_or_create_server_keypair() {
        Ok((pk, _)) => tracing::info!("Server public key: {pk}"),
        Err(e) => tracing::error!("Failed to initialize server keypair: {e}"),
    }

    // Ensure default channel exists.
    db.ensure_default_channel().expect("Failed to create default channel");

    // Create additional default channels (read-only where noted).
    let _ = db.create_channel("welcome", "welcome", Some("Welcome to Humanity Network"), "system", true);
    let _ = db.create_channel("announcements", "announcements", Some("Project updates and news"), "system", true);
    let _ = db.create_channel("rules", "rules", Some("Community guidelines"), "system", true);
    let _ = db.create_channel("general", "general", Some("General discussion"), "system", false);
    let _ = db.create_channel("stream", "stream", Some("Live streams and stream chat -- visible without opening a stream"), "system", false);
    let _ = db.create_channel("dev", "dev", Some("Development discussion"), "system", false);

    // Set channel display order: welcome(0), rules(1), announcements(2), general(10), stream(15), dev(20).
    let _ = db.set_channel_position("welcome", 0);
    let _ = db.set_channel_position("rules", 1);
    let _ = db.set_channel_position("announcements", 2);
    let _ = db.set_channel_position("general", 10);
    let _ = db.set_channel_position("stream", 15);
    let _ = db.set_channel_position("dev", 20);

    let mut relay_state = RelayState::new(db);
    relay_state.init_vapid_key();
    let state = Arc::new(relay_state);

    // Federation Phase 2: start outbound connections to verified federated servers.
    {
        let fed_state = state.clone();
        tokio::spawn(async move {
            // Small delay to let the server finish starting.
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            let count = relay::start_federation_connections(&fed_state).await;
            if count > 0 {
                tracing::info!("Federation: initiated connections to {} servers", count);
            }
        });
    }

    // Game world simulation tick loop: 20 ticks/sec (50ms), only when players connected.
    {
        let game_state = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(50));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                let mut world = game_state.game_world.write().await;
                if world.player_count() > 0 {
                    world.tick(0.05); // 50ms = 0.05 seconds
                }
                drop(world);
            }
        });
    }

    // Game TimeSync broadcast: every 5 seconds, only when players connected.
    {
        let game_state = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
            loop {
                interval.tick().await;
                let world = game_state.game_world.read().await;
                let player_count = world.player_count();
                let game_time = world.game_time;
                drop(world);

                if player_count > 0 {
                    let server_time = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs_f64();
                    let sync_msg = serde_json::json!({
                        "type": "game_time_sync",
                        "game_time": game_time,
                        "server_time": server_time,
                    });
                    let _ = game_state.broadcast_tx.send(relay::RelayMessage::System {
                        message: format!("__game__:{}", sync_msg),
                    });
                }
            }
        });
    }

    // Automated SQLite backup every 6 hours, keeping last 5 backups.
    {
        let backup_db_path = db_path.clone();
        tokio::spawn(async move {
            use std::path::PathBuf;

            let backup_dir = PathBuf::from(&backup_db_path)
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .join("backups");

            loop {
                // Wait 6 hours between backups.
                tokio::time::sleep(tokio::time::Duration::from_secs(6 * 60 * 60)).await;

                // Ensure backup directory exists.
                if let Err(e) = std::fs::create_dir_all(&backup_dir) {
                    tracing::error!("DB backup: failed to create backup directory: {e}");
                    continue;
                }

                // Generate timestamped backup filename.
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                // Convert epoch seconds to YYYYMMDD_HHMMSS.
                let secs_per_day = 86400;
                let days = now / secs_per_day;
                let time_of_day = now % secs_per_day;
                // Simple date calculation (accurate enough for filenames).
                let (year, month, day) = epoch_days_to_ymd(days as i64);
                let hours = time_of_day / 3600;
                let minutes = (time_of_day % 3600) / 60;
                let seconds = time_of_day % 60;

                let backup_filename = format!(
                    "relay_{:04}{:02}{:02}_{:02}{:02}{:02}.db",
                    year, month, day, hours, minutes, seconds
                );
                let backup_path = backup_dir.join(&backup_filename);

                // Use SQLite VACUUM INTO for a consistent backup.
                match rusqlite::Connection::open(&backup_db_path) {
                    Ok(conn) => {
                        let vacuum_sql = format!("VACUUM INTO '{}'", backup_path.display());
                        match conn.execute_batch(&vacuum_sql) {
                            Ok(_) => tracing::info!("DB backup: created {backup_filename}"),
                            Err(e) => {
                                tracing::error!("DB backup: VACUUM INTO failed: {e}");
                                continue;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("DB backup: failed to open database: {e}");
                        continue;
                    }
                }

                // Prune old backups, keeping the last 5.
                if let Ok(entries) = std::fs::read_dir(&backup_dir) {
                    let mut backups: Vec<PathBuf> = entries
                        .filter_map(|e| e.ok())
                        .map(|e| e.path())
                        .filter(|p| {
                            p.extension().and_then(|e| e.to_str()) == Some("db")
                                && p.file_name()
                                    .and_then(|n| n.to_str())
                                    .map(|n| n.starts_with("relay_"))
                                    .unwrap_or(false)
                        })
                        .collect();
                    backups.sort();
                    while backups.len() > 5 {
                        let oldest = backups.remove(0);
                        if let Err(e) = std::fs::remove_file(&oldest) {
                            tracing::warn!("DB backup: failed to remove old backup {}: {e}", oldest.display());
                        } else {
                            tracing::info!("DB backup: pruned old backup {}", oldest.file_name().unwrap_or_default().to_string_lossy());
                        }
                    }
                }
            }
        });
    }

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/health", get(health))
        // Bot HTTP API
        .route("/api/send", post(api::send_message))
        .route("/api/messages", get(api::get_messages))
        .route("/api/peers", get(api::get_peers))
        .route("/api/stats", get(api::get_stats))
        .route("/api/reactions", get(api::get_reactions))
        .route("/api/pins", get(api::get_pins))
        .route("/api/upload", post(api::upload_file))
        .route("/api/github-webhook", post(api::github_webhook))
        .route("/api/tasks", get(api::get_tasks).post(api::create_task))
        .route("/api/tasks/{id}", patch(api::update_task).delete(api::delete_task))
        .route("/api/tasks/{id}/comments", get(api::get_task_comments).post(api::create_task_comment))
        .route("/api/members", get(api::get_members))
        .route("/api/members/count", get(api::get_member_count))
        .route("/api/members/{key}", get(api::get_member_by_key))
        .route("/api/server-info", get(api::get_server_info))
        .route("/api/civilization", get(api::get_civilization_stats))
        .route("/api/profile/{key}", get(api::get_signed_profile))
        .route("/api/assets", get(api::get_assets).post(api::create_asset))
        .route("/api/assets/{id}", delete(api::delete_asset))
        .route("/api/projects", get(api::get_projects).post(api::create_project))
        .route("/api/projects/{id}", patch(api::update_project).delete(api::delete_project))
        .route("/api/listings", get(api::get_listings).post(api::create_listing))
        .route("/api/listings/{id}/images", get(api::get_listing_images).post(api::add_listing_image))
        .route("/api/listings/{listing_id}/images/{image_id}", delete(api::delete_listing_image))
        .route("/api/listings/{id}/reviews", get(api::get_listing_reviews).post(api::create_listing_review))
        .route("/api/listings/{id}/reviews/{review_id}", delete(api::delete_listing_review))
        .route("/api/sellers/{key}/rating", get(api::get_seller_rating))
        // Order-book trading
        .route("/api/trade/orders", get(api::get_trade_orders).post(api::create_trade_order))
        .route("/api/trade/orders/{id}", delete(api::cancel_trade_order))
        .route("/api/trade/orders/{id}/fill", post(api::fill_trade_order))
        .route("/api/trade/history", get(api::get_trade_history))
        // Guilds
        .route("/api/guilds", get(api::get_guilds).post(api::create_guild))
        .route("/api/guilds/{id}", get(api::get_guild).patch(api::update_guild).delete(api::delete_guild))
        .route("/api/guilds/{id}/members", get(api::get_guild_members).post(api::join_guild))
        .route("/api/guilds/{id}/leave", post(api::leave_guild))
        .route("/api/guilds/{id}/invite", post(api::create_guild_invite))
        // Bug reports
        .route("/api/bugs", get(api::get_bugs).post(api::create_bug))
        .route("/api/bugs/{id}", patch(api::update_bug_status))
        .route("/api/bugs/{id}/vote", post(api::vote_bug))
        // Reputation
        .route("/api/reputation/leaderboard", get(api::get_reputation_leaderboard))
        .route("/api/reputation/{key}", get(api::get_reputation))
        .route("/api/federation/servers", get(api::list_federation_servers))
        .route("/api/search", get(api::search_messages))
        .route("/api/skills/search", get(api::search_skills))
        .route("/api/skills/{user_key}", get(api::get_user_skills))
        .route("/api/vault/sync",
            get(api::vault_sync_get)
            .put(api::vault_sync_put)
            .delete(api::vault_sync_delete)
        )
        .route("/api/push/subscribe", post(api::push_subscribe))
        .route("/api/push/unsubscribe", post(api::push_unsubscribe))
        .route("/api/vapid-public-key", get(api::get_vapid_public_key))
        .route("/api/files", get(api::list_files))
        .route("/api/files/read", get(api::read_file))
        .route("/api/files/write", post(api::write_file))
        .route("/api/admin/stats", get(api::get_admin_stats))
        .route("/api/asset-manifest", get(api::get_asset_manifest))
        .route("/api/web-manifest", get(api::get_web_manifest))
        // === API v2: signed objects substrate (Phase 0 PR 2) ===
        .route("/api/v2/objects", get(api_v2_objects::list_objects).post(api_v2_objects::post_object))
        .route("/api/v2/objects/count", get(api_v2_objects::count_objects))
        .route("/api/v2/objects/{object_id}", get(api_v2_objects::get_object_by_id))
        // === API v2: DID resolver (Phase 1 PR 1) ===
        .route("/api/v2/did/{did}", get(api_v2_did::resolve_did))
        // === API v2: Verifiable Credentials (Phase 1 PR 2) ===
        .route("/api/v2/credentials", get(api_v2_credentials::list_credentials))
        .route("/api/v2/credentials/{vc_object_id}", get(api_v2_credentials::get_credential))
        // === API v2: Trust score (Phase 2 PR 1) ===
        .route("/api/v2/trust/{did}", get(api_v2_trust::get_trust_score))
        // === API v2: Governance (Phase 5 PR 1) ===
        .route("/api/v2/proposals", get(api_v2_governance::list_proposals))
        .route("/api/v2/proposals/{id}", get(api_v2_governance::get_proposal))
        .route("/api/v2/proposals/{id}/tally", get(api_v2_governance::tally_proposal))
        // === API v2: AI-as-citizen status (Phase 8 PR 1) ===
        .route("/api/v2/ai-status/{did}", get(api_v2_ai::get_ai_status))
        // === API v2: Social key recovery (Phase 4 PR 1) ===
        .route("/api/v2/recovery/setup/{holder_did}", get(api_v2_recovery::get_recovery_setup))
        .route("/api/v2/recovery/shares-held-by/{guardian_did}", get(api_v2_recovery::get_shares_held_by))
        .route("/api/me/system",
            get(api::system_profile_get)
            .put(api::system_profile_put)
            .delete(api::system_profile_delete)
        )
        .nest_service("/uploads", tower_http::services::ServeDir::new("data/uploads"))
        .fallback_service(
            tower_http::services::ServeDir::new("client")
                .fallback(tower_http::services::ServeFile::new("client/index.html")),
        )
        .layer(middleware::from_fn(security_headers))
        .layer(
            CorsLayer::new()
                .allow_origin([
                    "https://chat.united-humanity.us".parse::<http::HeaderValue>().unwrap(),
                    "https://united-humanity.us".parse::<http::HeaderValue>().unwrap(),
                    // Tauri 2 desktop app
                    "http://tauri.localhost".parse::<http::HeaderValue>().unwrap(),
                    "https://tauri.localhost".parse::<http::HeaderValue>().unwrap(),
                    "tauri://localhost".parse::<http::HeaderValue>().unwrap(),
                ])
                .allow_methods([http::Method::GET, http::Method::POST, http::Method::PUT, http::Method::DELETE, http::Method::PATCH])
                .allow_headers([http::header::CONTENT_TYPE, http::header::AUTHORIZATION])
        )
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    tracing::info!("Humanity relay listening on {addr}");
    tracing::info!("Web client: http://localhost:{port}");
    tracing::info!("WebSocket:  ws://localhost:{port}/ws");
    tracing::info!("Bot API:    http://localhost:{port}/api/");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health(
    state: axum::extract::State<Arc<RelayState>>,
) -> axum::Json<serde_json::Value> {
    let uptime = state.start_time.elapsed().as_secs();
    let msg_count = state.db.message_count().unwrap_or(0);
    let peers = state.peers.read().await.len();
    axum::Json(serde_json::json!({
        "status": "ok",
        "uptime_seconds": uptime,
        "total_messages": msg_count,
        "connected_peers": peers,
    }))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    state: axum::extract::State<Arc<RelayState>>,
) -> impl IntoResponse {
    // Check Origin header for browser connections.
    // Non-browser clients (native apps, bots) typically don't send Origin,
    // so we only reject when Origin is present but not in the allow-list.
    if let Some(origin) = headers.get("origin").and_then(|v| v.to_str().ok()) {
        let allowed = [
            "https://chat.united-humanity.us",
            "https://united-humanity.us",
            "http://tauri.localhost",
            "https://tauri.localhost",
            "tauri://localhost",
        ];
        if !allowed.iter().any(|&a| a == origin) {
            return (StatusCode::FORBIDDEN, "Origin not allowed").into_response();
        }
    }
    ws.max_frame_size(65_536)       // 64KB max frame
      .max_message_size(131_072)    // 128KB max message
      .on_upgrade(move |socket| handle_socket(socket, state.0))
      .into_response()
}

async fn handle_socket(socket: WebSocket, state: Arc<RelayState>) {
    relay::handle_connection(socket, state).await;
}
