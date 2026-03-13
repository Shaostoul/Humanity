//! Server-to-server federation functions extracted from relay.rs.
//! Handles outbound WebSocket connections to peer servers and message forwarding.

use std::sync::Arc;
use std::time::Instant;

use futures::{SinkExt, StreamExt};

use crate::relay::{FederatedConnection, FederationServerStatus, RelayMessage, RelayState};
use crate::storage::Storage;

/// Sign a message with the server's Ed25519 key.
pub fn sign_with_server_key(db: &Storage, message: &str) -> Option<String> {
    let (_, sk_hex) = db.get_or_create_server_keypair().ok()?;
    let sk_bytes = hex::decode(&sk_hex).ok()?;
    if sk_bytes.len() != 32 { return None; }
    let sk_array: [u8; 32] = sk_bytes.try_into().ok()?;
    use ed25519_dalek::{Signer, SigningKey};
    let signing_key = SigningKey::from_bytes(&sk_array);
    let sig = signing_key.sign(message.as_bytes());
    Some(hex::encode(sig.to_bytes()))
}

/// Forward a chat message to all connected federated servers.
pub async fn forward_to_federation(state: &Arc<RelayState>, channel: &str, from_name: &str, content: &str, timestamp: u64) {
    let (server_id, _) = match state.db.get_or_create_server_keypair() {
        Ok(kp) => kp,
        Err(_) => return,
    };
    let server_name = std::env::var("SERVER_NAME").unwrap_or_else(|_| "Humanity Relay".to_string());

    // Sign the message for authenticity.
    let sig_message = format!("{}\n{}\n{}", content, timestamp, channel);
    let signature = sign_with_server_key(&state.db, &sig_message);

    let federated_msg = RelayMessage::FederatedChat {
        server_id: server_id.clone(),
        server_name,
        from_name: from_name.to_string(),
        content: content.to_string(),
        timestamp,
        channel: channel.to_string(),
        signature,
    };

    let json = match serde_json::to_string(&federated_msg) {
        Ok(j) => j,
        Err(_) => return,
    };

    let connections = state.federation_connections.read().await;
    for conn in connections.values() {
        // Rate limit: max 10 messages per second per server.
        let allow = {
            let mut rate = state.federation_rate.lock().unwrap();
            let times = rate.entry(conn.server_id.clone()).or_default();
            let now = Instant::now();
            times.retain(|t| now.duration_since(*t).as_secs() < 1);
            if times.len() < 10 {
                times.push(now);
                true
            } else {
                false
            }
        };
        if allow {
            let _ = conn.tx.send(json.clone());
        }
    }
}

/// Start outbound WebSocket connections to all verified federated servers.
/// Returns the number of connection attempts initiated.
pub async fn start_federation_connections(state: &Arc<RelayState>) -> usize {
    let servers = match state.db.list_federated_servers() {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let mut count = 0;
    for server in servers {
        if server.trust_tier < 2 { continue; }

        let ws_url = {
            let base = server.url.trim_end_matches('/');
            let ws_base = if base.starts_with("https://") {
                base.replacen("https://", "wss://", 1)
            } else if base.starts_with("http://") {
                base.replacen("http://", "ws://", 1)
            } else {
                continue;
            };
            format!("{}/ws", ws_base)
        };

        let state_clone = state.clone();
        let server_id = server.server_id.clone();
        let server_name = server.name.clone();
        let trust_tier = server.trust_tier;

        tokio::spawn(async move {
            federation_connect_loop(state_clone, server_id, server_name, trust_tier, ws_url).await;
        });
        count += 1;
    }
    count
}

/// Connect to a single federated server with exponential backoff reconnection.
pub async fn federation_connect_loop(
    state: Arc<RelayState>,
    server_id: String,
    server_name: String,
    trust_tier: i32,
    ws_url: String,
) {
    let mut backoff_secs = 5u64;
    loop {
        tracing::info!("Federation: connecting to {} ({})", server_name, ws_url);
        match tokio_tungstenite::connect_async(&ws_url).await {
            Ok((ws_stream, _)) => {
                backoff_secs = 5; // Reset on successful connect.
                tracing::info!("Federation: connected to {}", server_name);

                let (mut write, mut read) = ws_stream.split();
                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

                // Register the connection.
                {
                    let mut conns = state.federation_connections.write().await;
                    conns.insert(server_id.clone(), FederatedConnection {
                        tx: tx.clone(),
                        server_id: server_id.clone(),
                        server_name: server_name.clone(),
                        trust_tier,
                        connected_at: Instant::now(),
                    });
                }
                let _ = state.db.update_federated_server_status(&server_id, "online");
                broadcast_federation_status(&state).await;

                // Send federation hello.
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let (our_pk, _) = state.db.get_or_create_server_keypair().unwrap_or_default();
                let our_name = std::env::var("SERVER_NAME").unwrap_or_else(|_| "Humanity Relay".to_string());
                let sig = sign_with_server_key(&state.db, &timestamp.to_string()).unwrap_or_default();

                let hello = RelayMessage::FederationHello {
                    server_id: our_pk.clone(),
                    public_key: our_pk.clone(),
                    name: our_name,
                    version: env!("BUILD_VERSION").to_string(),
                    timestamp,
                    signature: sig,
                };
                if let Ok(json) = serde_json::to_string(&hello) {
                    use tokio_tungstenite::tungstenite::Message as TMessage;
                    let _ = write.send(TMessage::Text(json.into())).await;
                }

                // Spawn write pump.
                let write_task = tokio::spawn(async move {
                    use tokio_tungstenite::tungstenite::Message as TMessage;
                    while let Some(msg) = rx.recv().await {
                        if write.send(TMessage::Text(msg.into())).await.is_err() {
                            break;
                        }
                    }
                });

                // Read pump: handle incoming messages from federated server.
                let state_for_read = state.clone();
                let _sid_for_read = server_id.clone();
                let read_task = tokio::spawn(async move {
                    use tokio_tungstenite::tungstenite::Message as TMessage;
                    while let Some(Ok(msg)) = read.next().await {
                        if let TMessage::Text(text) = msg {
                            if let Ok(relay_msg) = serde_json::from_str::<RelayMessage>(&text) {
                                match relay_msg {
                                    RelayMessage::FederatedChat { server_id, server_name, from_name, content, timestamp, channel, signature } => {
                                        // Only deliver to locally federated channels.
                                        if state_for_read.db.is_channel_federated(&channel).unwrap_or(false) {
                                            let _ = state_for_read.broadcast_tx.send(RelayMessage::FederatedChat {
                                                server_id, server_name, from_name, content, timestamp, channel, signature,
                                            });
                                        }
                                    }
                                    RelayMessage::FederationWelcome { server_id, name, channels } => {
                                        tracing::info!("Federation: welcome from {} — channels: {:?}", name, channels);
                                        let _ = state_for_read.db.update_federated_server_status(&server_id, "online");
                                    }
                                    _ => {} // Ignore other message types from federation.
                                }
                            }
                        }
                    }
                });

                // Wait for either to finish (connection dropped).
                tokio::select! {
                    _ = write_task => {}
                    _ = read_task => {}
                }

                // Clean up.
                {
                    let mut conns = state.federation_connections.write().await;
                    conns.remove(&server_id);
                }
                let _ = state.db.update_federated_server_status(&server_id, "disconnected");
                broadcast_federation_status(&state).await;
                tracing::warn!("Federation: disconnected from {}", server_name);
            }
            Err(e) => {
                tracing::warn!("Federation: failed to connect to {} ({}): {}", server_name, ws_url, e);
                let _ = state.db.update_federated_server_status(&server_id, "unreachable");
            }
        }

        // Exponential backoff (cap at 5 minutes).
        tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs)).await;
        backoff_secs = (backoff_secs * 2).min(300);
    }
}

/// Broadcast federation status to all connected clients.
pub async fn broadcast_federation_status(state: &Arc<RelayState>) {
    let servers = state.db.list_federated_servers().unwrap_or_default();
    let connections = state.federation_connections.read().await;

    let statuses: Vec<FederationServerStatus> = servers.iter().map(|s| {
        let connected = connections.contains_key(&s.server_id);
        FederationServerStatus {
            server_id: s.server_id.clone(),
            name: s.name.clone(),
            connected,
            trust_tier: s.trust_tier,
            peer_count: None,
        }
    }).collect();

    let _ = state.broadcast_tx.send(RelayMessage::FederationStatus { servers: statuses });
}
