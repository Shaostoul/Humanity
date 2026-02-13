//! Humanity Network relay server.
//!
//! A WebSocket relay that routes signed objects between connected clients.
//! This is the mandatory fallback transport defined in the hybrid network
//! architecture (design/architecture_decisions/hybrid_network.md).

mod relay;
mod api;
mod storage;

use axum::{
    Router,
    routing::{get, post, delete, patch},
    extract::ws::{WebSocket, WebSocketUpgrade},
    response::IntoResponse,
};
use axum::http::{self, HeaderMap, StatusCode};
use tower_http::cors::CorsLayer;
use tracing_subscriber::EnvFilter;
use std::sync::Arc;

use relay::RelayState;
use serde_json;

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // Initialize persistent storage.
    let db_path = std::env::var("DB_PATH")
        .unwrap_or_else(|_| "data/relay.db".to_string());
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
    let _ = db.create_channel("dev", "dev", Some("Development discussion"), "system", false);

    // Set channel display order: welcome(0), rules(1), announcements(2), general(10), dev(20).
    let _ = db.set_channel_position("welcome", 0);
    let _ = db.set_channel_position("rules", 1);
    let _ = db.set_channel_position("announcements", 2);
    let _ = db.set_channel_position("general", 10);
    let _ = db.set_channel_position("dev", 20);

    let state = Arc::new(RelayState::new(db));

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
        .route("/api/tasks/:id", patch(api::update_task).delete(api::delete_task))
        .route("/api/server-info", get(api::get_server_info))
        .route("/api/assets", get(api::get_assets).post(api::create_asset))
        .route("/api/assets/:id", delete(api::delete_asset))
        .route("/api/listings", get(api::get_listings).post(api::create_listing))
        .route("/api/federation/servers", get(api::list_federation_servers))
        .route("/api/search", get(api::search_messages))
        .nest_service("/uploads", tower_http::services::ServeDir::new("data/uploads"))
        .fallback_service(
            tower_http::services::ServeDir::new("client")
                .fallback(tower_http::services::ServeFile::new("client/index.html")),
        )
        .layer(
            CorsLayer::new()
                .allow_origin([
                    "https://chat.united-humanity.us".parse::<http::HeaderValue>().unwrap(),
                    "https://united-humanity.us".parse::<http::HeaderValue>().unwrap(),
                ])
                .allow_methods([http::Method::GET, http::Method::POST])
                .allow_headers([http::header::CONTENT_TYPE, http::header::AUTHORIZATION])
        )
        .with_state(state);

    let addr = "0.0.0.0:3210";
    tracing::info!("Humanity relay listening on {addr}");
    tracing::info!("Web client: http://localhost:3210");
    tracing::info!("WebSocket:  ws://localhost:3210/ws");
    tracing::info!("Bot API:    http://localhost:3210/api/");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
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
        let allowed = ["https://chat.united-humanity.us", "https://united-humanity.us"];
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
