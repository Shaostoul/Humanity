//! Humanity Network relay server.
//!
//! A WebSocket relay that routes signed objects between connected clients.
//! This is the mandatory fallback transport defined in the hybrid network
//! architecture (design/architecture_decisions/hybrid_network.md).

mod relay;
mod api;

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

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let state = Arc::new(RelayState::new());

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/health", get(health))
        // Bot HTTP API
        .route("/api/send", post(api::send_message))
        .route("/api/messages", get(api::get_messages))
        .route("/api/peers", get(api::get_peers))
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

async fn health() -> &'static str {
    "ok"
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
