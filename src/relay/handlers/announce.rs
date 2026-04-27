//! Server-side announcements (v0.119.0).
//!
//! Posts a chat message signed by the server's own Ed25519 keypair into a
//! configured channel (default: "announcements"). Connected clients see it
//! live via the existing broadcast channel; the message is also persisted
//! so anyone joining later sees the history.
//!
//! Hooks:
//!   - `announce(state, content)` — convenience: posts to the default channel.
//!   - `announce_to(state, channel, content)` — explicit channel.
//!
//! Auto-fire points wired in v0.119.0:
//!   - Agent override set via POST /api/v2/agents/override
//!   - Agent claim/release on the agent_sessions table (best-effort spawn)
//!   - External callers via POST /api/v2/announce (admin-auth)

use std::sync::Arc;

use crate::relay::handlers::federation::sign_with_server_key;
use crate::relay::relay::{RelayMessage, RelayState};

/// Default channel for system announcements. Auto-created on first post via
/// the same path normal channel creation takes (insert into channels table
/// if missing).
pub const DEFAULT_ANNOUNCEMENT_CHANNEL: &str = "announcements";

/// Post an announcement to the default channel.
pub async fn announce(state: &Arc<RelayState>, content: &str) {
    announce_to(state, DEFAULT_ANNOUNCEMENT_CHANNEL, content).await;
}

/// Post an announcement to an explicit channel. Best-effort: failures are
/// logged but never propagated — announcements are observability, not
/// correctness.
pub async fn announce_to(state: &Arc<RelayState>, channel: &str, content: &str) {
    let server_name = std::env::var("SERVER_NAME")
        .unwrap_or_else(|_| "HumanityOS Relay".to_string());

    // Server identity = its own Ed25519 keypair (legacy, used for federation
    // and now system messages). The server's public key uniquely identifies
    // this relay across the network.
    let (server_pubkey_hex, _) = match state.db.get_or_create_server_keypair() {
        Ok(kp) => kp,
        Err(e) => {
            tracing::warn!("announce: failed to load server keypair: {e}");
            return;
        }
    };

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    // Sign "{content}\n{timestamp}" — same convention as user chat messages.
    let sig_message = format!("{}\n{}", content, timestamp);
    let signature = sign_with_server_key(&state.db, &sig_message);

    // Auto-create the channel if missing. The channels table only requires
    // an id + name; we skip if it already exists.
    if let Err(e) = state.db.with_conn(|conn| {
        conn.execute(
            "INSERT OR IGNORE INTO channels (id, name, description, created_by, created_at, read_only)
             VALUES (?1, ?2, 'System announcements', ?3, ?4, 0)",
            rusqlite::params![channel, channel, &server_pubkey_hex, timestamp as i64],
        )
    }) {
        tracing::warn!("announce: channel autocreate failed: {e}");
    }

    let chat = RelayMessage::Chat {
        from: server_pubkey_hex.clone(),
        from_name: Some(format!("\u{1F4E2} {}", server_name)), // 📢 prefix marks system
        content: content.to_string(),
        timestamp,
        signature,
        channel: channel.to_string(),
        reply_to: None,
        thread_count: None,
        message_id: None,
    };

    // Persist to messages table.
    if let Err(e) = state.db.store_message(&chat) {
        tracing::warn!("announce: store_message failed: {e}");
    }

    // Broadcast to connected clients.
    let _ = state.broadcast_tx.send(chat);
    tracing::info!("Announce[{}]: {}", channel, content);
}

/// Spawn an announcement on a tokio task so callers don't block waiting for
/// the broadcast/storage. Use this from sync code paths or when you don't
/// want to await an announcement.
pub fn announce_async(state: Arc<RelayState>, channel: String, content: String) {
    tokio::spawn(async move {
        announce_to(&state, &channel, &content).await;
    });
}
