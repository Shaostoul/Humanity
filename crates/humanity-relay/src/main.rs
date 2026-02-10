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
    routing::{get, post},
    extract::ws::{WebSocket, WebSocketUpgrade},
    response::IntoResponse,
};
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

    let state = Arc::new(RelayState::new(db));

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/health", get(health))
        // Bot HTTP API
        .route("/api/send", post(api::send_message))
        .route("/api/messages", get(api::get_messages))
        .route("/api/peers", get(api::get_peers))
        .route("/api/stats", get(api::get_stats))
        .route("/api/upload", post(api::upload_file))
        .nest_service("/uploads", tower_http::services::ServeDir::new("data/uploads"))
        .fallback_service(
            tower_http::services::ServeDir::new("client")
                .fallback(tower_http::services::ServeFile::new("client/index.html")),
        )
        .layer(CorsLayer::permissive())
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
    state: axum::extract::State<Arc<RelayState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state.0))
}

async fn handle_socket(socket: WebSocket, state: Arc<RelayState>) {
    relay::handle_connection(socket, state).await;
}
