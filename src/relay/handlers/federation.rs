//! Server-to-server federation functions extracted from relay.rs.
//! Handles outbound WebSocket connections to peer servers and message forwarding.

use std::sync::Arc;
use std::time::Instant;

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use futures::{SinkExt, StreamExt};

use crate::relay::core::object::Object;
use crate::relay::relay::{FederatedConnection, FederationServerStatus, RelayMessage, RelayState};
use crate::relay::storage::Storage;

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

/// Canonical message bytes that a profile signature commits to.
///
/// Format (newline-separated, type-tagged for forward versioning):
///   profile_v1\n{public_key}\n{name}\n{bio}\n{avatar_url}\n{banner_url}\n{socials}\n{pronouns}\n{location}\n{website}\n{timestamp}
///
/// Any field change invalidates the signature; the version tag lets us
/// rotate the format without ambiguity if profile shape evolves.
#[allow(clippy::too_many_arguments)]
pub fn canonical_profile_message(
    public_key: &str,
    name: &str,
    bio: &str,
    avatar_url: &str,
    banner_url: &str,
    socials: &str,
    pronouns: &str,
    location: &str,
    website: &str,
    timestamp: u64,
) -> String {
    format!(
        "profile_v1\n{public_key}\n{name}\n{bio}\n{avatar_url}\n{banner_url}\n{socials}\n{pronouns}\n{location}\n{website}\n{timestamp}"
    )
}

/// Verify an Ed25519 signature over a profile gossip payload.
/// Returns true only when both the public key and signature decode and the
/// signature commits to the canonical message bytes.
#[allow(clippy::too_many_arguments)]
pub fn verify_profile_signature(
    public_key_hex: &str,
    name: &str,
    bio: &str,
    avatar_url: &str,
    banner_url: &str,
    socials: &str,
    pronouns: &str,
    location: &str,
    website: &str,
    timestamp: u64,
    signature_hex: &str,
) -> bool {
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};

    let Ok(pk_bytes) = hex::decode(public_key_hex) else { return false };
    if pk_bytes.len() != 32 { return false; }
    let pk_array: [u8; 32] = match pk_bytes.try_into() { Ok(a) => a, Err(_) => return false };
    let Ok(verifying_key) = VerifyingKey::from_bytes(&pk_array) else { return false };

    let Ok(sig_bytes) = hex::decode(signature_hex) else { return false };
    if sig_bytes.len() != 64 { return false; }
    let sig_array: [u8; 64] = match sig_bytes.try_into() { Ok(a) => a, Err(_) => return false };
    let signature = Signature::from_bytes(&sig_array);

    let message = canonical_profile_message(
        public_key_hex, name, bio, avatar_url, banner_url, socials,
        pronouns, location, website, timestamp,
    );
    verifying_key.verify(message.as_bytes(), &signature).is_ok()
}

/// Decide whether to accept an inbound `ProfileGossip` payload.
/// Returns true when the gossip should be stored.
///
/// Signed clients (non-empty `signature_hex`) MUST verify. A bad signature
/// from a signed client is treated as forgery and rejected.
///
/// Unsigned clients (empty `signature_hex`) are accepted under the
/// trust-by-source model: the gossip arrived over an authenticated
/// federation link, so we trust the peer server's vetting until clients
/// gain their own profile-signing path. When that lands, this gate flips
/// to a hard reject for unsigned profiles.
#[allow(clippy::too_many_arguments)]
pub fn should_accept_profile_gossip(
    public_key_hex: &str,
    name: &str,
    bio: &str,
    avatar_url: &str,
    banner_url: &str,
    socials: &str,
    pronouns: &str,
    location: &str,
    website: &str,
    timestamp: u64,
    signature_hex: &str,
) -> bool {
    if signature_hex.is_empty() {
        return true;
    }
    verify_profile_signature(
        public_key_hex, name, bio, avatar_url, banner_url, socials,
        pronouns, location, website, timestamp, signature_hex,
    )
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
                                            // Persist the federated message so it survives restarts.
                                            let msg_json = serde_json::json!({
                                                "type": "federated_chat",
                                                "server_id": &server_id,
                                                "server_name": &server_name,
                                                "from_name": &from_name,
                                                "content": &content,
                                                "timestamp": timestamp,
                                                "channel": &channel,
                                            });
                                            let _ = state_for_read.db.store_federated_message(
                                                &channel,
                                                &from_name,
                                                &server_id,
                                                &content,
                                                timestamp,
                                                &msg_json.to_string(),
                                                &server_id,
                                            );

                                            let _ = state_for_read.broadcast_tx.send(RelayMessage::FederatedChat {
                                                server_id, server_name, from_name, content, timestamp, channel, signature,
                                            });
                                        }
                                    }
                                    RelayMessage::FederationWelcome { server_id, name, channels } => {
                                        tracing::info!("Federation: welcome from {} — channels: {:?}", name, channels);
                                        let _ = state_for_read.db.update_federated_server_status(&server_id, "online");
                                    }
                                    // Profile gossip from federated server — cache if newer.
                                    RelayMessage::ProfileGossip { public_key, name, bio, avatar_url, banner_url, socials, pronouns, location, website, timestamp, signature } => {
                                        if !should_accept_profile_gossip(
                                            &public_key, &name, &bio, &avatar_url, &banner_url,
                                            &socials, &pronouns, &location, &website,
                                            timestamp, &signature,
                                        ) {
                                            tracing::warn!("Federation: rejecting profile gossip for {} — signature did not verify", &name);
                                            continue;
                                        }
                                        tracing::debug!("Federation: received profile gossip for {}", &name);
                                        let _ = state_for_read.db.store_signed_profile(
                                            &public_key,
                                            &name,
                                            &bio,
                                            &avatar_url,
                                            &banner_url,
                                            &socials,
                                            &pronouns,
                                            &location,
                                            &website,
                                            timestamp,
                                            &signature,
                                        );
                                    }
                                    // Generic post-quantum signed-object gossip (Phase 3 PR 1).
                                    RelayMessage::SignedObjectGossip {
                                        object_id, protocol_version, object_type,
                                        space_id, channel_id, author_public_key_b64,
                                        created_at, references, payload_schema_version,
                                        payload_encoding, payload_b64, signature_b64,
                                    } => {
                                        let author_public_key = match B64.decode(&author_public_key_b64) {
                                            Ok(b) => b,
                                            Err(_) => {
                                                tracing::warn!(
                                                    "Federation: invalid base64 in author_public_key_b64 from {}",
                                                    _sid_for_read
                                                );
                                                continue;
                                            }
                                        };
                                        let payload = match B64.decode(&payload_b64) {
                                            Ok(b) => b,
                                            Err(_) => continue,
                                        };
                                        let signature = match B64.decode(&signature_b64) {
                                            Ok(b) => b,
                                            Err(_) => continue,
                                        };

                                        let object = Object {
                                            protocol_version,
                                            object_type: object_type.clone(),
                                            space_id,
                                            channel_id,
                                            author_public_key,
                                            created_at,
                                            references,
                                            payload_schema_version,
                                            payload_encoding,
                                            payload,
                                            signature,
                                        };

                                        // put_signed_object verifies the Dilithium3 signature.
                                        let source = Some(_sid_for_read.as_str());
                                        match state_for_read.db.put_signed_object(&object, source) {
                                            Ok(true) => {
                                                tracing::debug!(
                                                    "Federation: accepted {} object {} from {}",
                                                    object_type, object_id, _sid_for_read
                                                );
                                                // Phase 3 PR 2: multi-hop gossip — re-emit to peers
                                                // OTHER than the source. INSERT OR IGNORE on the
                                                // receiving side breaks any cycles.
                                                let state_for_gossip = state_for_read.clone();
                                                let object_for_gossip = object.clone();
                                                let exclude = _sid_for_read.clone();
                                                tokio::spawn(async move {
                                                    gossip_signed_object(
                                                        &state_for_gossip,
                                                        &object_for_gossip,
                                                        Some(&exclude),
                                                    )
                                                    .await;
                                                });
                                            }
                                            Ok(false) => {
                                                // Already had this object — gossip convergence; do not re-emit.
                                            }
                                            Err(e) => {
                                                tracing::warn!(
                                                    "Federation: rejected {} object from {}: {}",
                                                    object_type, _sid_for_read, e
                                                );
                                            }
                                        }
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

/// Gossip a post-quantum signed object to all connected federated servers
/// (Phase 3 PR 1+2). Called from the API after a local user submits a new
/// signed_object, AND from the federation receiver after accepting a peer's
/// gossip (multi-hop propagation, Phase 3 PR 2).
///
/// `exclude_server_id`: skip this peer when re-gossiping. Used to avoid
/// echoing back to the source. Loops are also broken by `INSERT OR IGNORE`
/// dedup on object_id — a peer that has already seen the object discards it.
pub async fn gossip_signed_object(
    state: &Arc<RelayState>,
    object: &Object,
    exclude_server_id: Option<&str>,
) {
    let object_id = match object.object_id() {
        Ok(h) => h.to_hex(),
        Err(_) => return,
    };
    let msg = RelayMessage::SignedObjectGossip {
        object_id: object_id.clone(),
        protocol_version: object.protocol_version,
        object_type: object.object_type.clone(),
        space_id: object.space_id.clone(),
        channel_id: object.channel_id.clone(),
        author_public_key_b64: B64.encode(&object.author_public_key),
        created_at: object.created_at,
        references: object.references.clone(),
        payload_schema_version: object.payload_schema_version,
        payload_encoding: object.payload_encoding.clone(),
        payload_b64: B64.encode(&object.payload),
        signature_b64: B64.encode(&object.signature),
    };

    let json = match serde_json::to_string(&msg) {
        Ok(j) => j,
        Err(_) => return,
    };

    let connections = state.federation_connections.read().await;
    let mut sent = 0;
    for conn in connections.values() {
        if let Some(exclude) = exclude_server_id {
            if conn.server_id == exclude {
                continue;
            }
        }
        // Per-peer rate limit: max 50 gossiped objects per second.
        let allow = {
            let mut rate = state.federation_rate.lock().unwrap();
            let times = rate.entry(format!("{}:obj", conn.server_id)).or_default();
            let now = Instant::now();
            times.retain(|t| now.duration_since(*t).as_secs() < 1);
            if times.len() < 50 {
                times.push(now);
                true
            } else {
                false
            }
        };
        if allow {
            let _ = conn.tx.send(json.clone());
            sent += 1;
        }
    }
    tracing::debug!(
        "Federation: gossiped {} object {} to {}/{} peer(s) (excluded={:?})",
        object.object_type,
        object_id,
        sent,
        connections.len(),
        exclude_server_id
    );
}

/// Gossip a profile update to all connected federated servers.
/// Called after a local user updates their profile. The `signature` is the
/// client-supplied Ed25519 signature over `canonical_profile_message(...)`;
/// pass an empty string when the client did not sign (peers will then accept
/// under the trust-by-source model — see `should_accept_profile_gossip`).
#[allow(clippy::too_many_arguments)]
pub async fn gossip_profile(
    state: &Arc<RelayState>,
    public_key: &str,
    name: &str,
    bio: &str,
    avatar_url: &str,
    banner_url: &str,
    socials: &str,
    pronouns: &str,
    location: &str,
    website: &str,
    timestamp: u64,
    signature: &str,
) {
    let gossip_msg = RelayMessage::ProfileGossip {
        public_key: public_key.to_string(),
        name: name.to_string(),
        bio: bio.to_string(),
        avatar_url: avatar_url.to_string(),
        banner_url: banner_url.to_string(),
        socials: socials.to_string(),
        pronouns: pronouns.to_string(),
        location: location.to_string(),
        website: website.to_string(),
        timestamp,
        signature: signature.to_string(),
    };

    let json = match serde_json::to_string(&gossip_msg) {
        Ok(j) => j,
        Err(_) => return,
    };

    let connections = state.federation_connections.read().await;
    for conn in connections.values() {
        let _ = conn.tx.send(json.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn fixture() -> (SigningKey, String) {
        let seed = [7u8; 32];
        let sk = SigningKey::from_bytes(&seed);
        let pk_hex = hex::encode(sk.verifying_key().to_bytes());
        (sk, pk_hex)
    }

    fn sign_profile(sk: &SigningKey, msg: &str) -> String {
        hex::encode(sk.sign(msg.as_bytes()).to_bytes())
    }

    #[test]
    fn verify_accepts_valid_signature() {
        let (sk, pk_hex) = fixture();
        let timestamp = 1_700_000_000_000u64;
        let msg = canonical_profile_message(
            &pk_hex, "Alice", "bio", "avatar", "banner", "socials",
            "pronouns", "location", "website", timestamp,
        );
        let sig_hex = sign_profile(&sk, &msg);

        assert!(verify_profile_signature(
            &pk_hex, "Alice", "bio", "avatar", "banner", "socials",
            "pronouns", "location", "website", timestamp, &sig_hex,
        ));
    }

    #[test]
    fn verify_rejects_forged_signature() {
        let (_sk, pk_hex) = fixture();
        let forged = "0".repeat(128); // 64 zero bytes hex-encoded
        assert!(!verify_profile_signature(
            &pk_hex, "Alice", "bio", "avatar", "banner", "socials",
            "pronouns", "location", "website", 1_700_000_000_000, &forged,
        ));
    }

    #[test]
    fn verify_rejects_tampered_field() {
        let (sk, pk_hex) = fixture();
        let timestamp = 1_700_000_000_000u64;
        let msg = canonical_profile_message(
            &pk_hex, "Alice", "bio", "avatar", "banner", "socials",
            "pronouns", "location", "website", timestamp,
        );
        let sig_hex = sign_profile(&sk, &msg);

        // Same signature, different name — should fail.
        assert!(!verify_profile_signature(
            &pk_hex, "Mallory", "bio", "avatar", "banner", "socials",
            "pronouns", "location", "website", timestamp, &sig_hex,
        ));
    }

    #[test]
    fn verify_rejects_wrong_key() {
        let (sk, _pk_hex) = fixture();
        let timestamp = 1_700_000_000_000u64;
        let other_seed = [9u8; 32];
        let other_sk = SigningKey::from_bytes(&other_seed);
        let other_pk_hex = hex::encode(other_sk.verifying_key().to_bytes());

        // Sign with one key, claim another's public key — should fail.
        let msg = canonical_profile_message(
            &other_pk_hex, "Alice", "bio", "avatar", "banner", "socials",
            "pronouns", "location", "website", timestamp,
        );
        let sig_hex = sign_profile(&sk, &msg);
        assert!(!verify_profile_signature(
            &other_pk_hex, "Alice", "bio", "avatar", "banner", "socials",
            "pronouns", "location", "website", timestamp, &sig_hex,
        ));
    }

    #[test]
    fn verify_rejects_malformed_inputs() {
        // Bad public key length.
        assert!(!verify_profile_signature(
            "deadbeef", "Alice", "bio", "avatar", "banner", "socials",
            "pronouns", "location", "website", 1, &"0".repeat(128),
        ));
        // Bad signature length.
        assert!(!verify_profile_signature(
            &"a".repeat(64), "Alice", "bio", "avatar", "banner", "socials",
            "pronouns", "location", "website", 1, "deadbeef",
        ));
        // Non-hex public key.
        assert!(!verify_profile_signature(
            "zz", "Alice", "bio", "avatar", "banner", "socials",
            "pronouns", "location", "website", 1, &"0".repeat(128),
        ));
    }

    #[test]
    fn accept_admits_empty_signature() {
        // Trust-by-source model: empty signature is admitted today.
        let (_sk, pk_hex) = fixture();
        assert!(should_accept_profile_gossip(
            &pk_hex, "Alice", "bio", "avatar", "banner", "socials",
            "pronouns", "location", "website", 1_700_000_000_000, "",
        ));
    }

    #[test]
    fn accept_admits_valid_signature() {
        let (sk, pk_hex) = fixture();
        let timestamp = 1_700_000_000_000u64;
        let msg = canonical_profile_message(
            &pk_hex, "Alice", "bio", "avatar", "banner", "socials",
            "pronouns", "location", "website", timestamp,
        );
        let sig_hex = sign_profile(&sk, &msg);
        assert!(should_accept_profile_gossip(
            &pk_hex, "Alice", "bio", "avatar", "banner", "socials",
            "pronouns", "location", "website", timestamp, &sig_hex,
        ));
    }

    #[test]
    fn accept_rejects_invalid_non_empty_signature() {
        let (_sk, pk_hex) = fixture();
        let forged = "0".repeat(128);
        assert!(!should_accept_profile_gossip(
            &pk_hex, "Alice", "bio", "avatar", "banner", "socials",
            "pronouns", "location", "website", 1_700_000_000_000, &forged,
        ));
    }
}
