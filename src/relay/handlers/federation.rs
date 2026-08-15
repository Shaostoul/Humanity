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

/// Verify a **Dilithium3** signature over a profile gossip payload.
/// Returns true only when both the public key and signature decode and
/// the signature commits to the canonical message bytes. Was Ed25519
/// before the full-PQ cutover; users' identity keys are Dilithium3
/// (1952-byte pubkey hex, 3309-byte sig hex), so an Ed25519 verify
/// here is forgeable post-quantum. Closes task #5.
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
    let Ok(pk_bytes) = hex::decode(public_key_hex) else { return false };
    let Ok(sig_bytes) = hex::decode(signature_hex) else { return false };
    let message = canonical_profile_message(
        public_key_hex, name, bio, avatar_url, banner_url, socials,
        pronouns, location, website, timestamp,
    );
    crate::relay::core::pq_crypto::verify_dilithium(&pk_bytes, message.as_bytes(), &sig_bytes).is_ok()
}

/// Decide whether to accept an inbound `ProfileGossip` payload.
/// Returns true when the gossip should be stored.
///
/// Every profile MUST carry a verifying Dilithium3 signature over the
/// canonical preimage: profiles are self-certifying, so the transport
/// (which peer relayed it) never has to be trusted for content. The old
/// accept-unsigned "trust-by-source" grace period is over; clients have
/// signed their profiles since the full-PQ cutover.
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
    // No signature, no cache write. The old fail-open here meant an empty
    // signature was a skeleton key: any message shaped like gossip could
    // overwrite any cached profile. A profile that cannot prove itself is
    // simply not replicated (closed 2026-08-13, federation repair).
    if signature_hex.is_empty() {
        return false;
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

    // Sign the message for authenticity. Canonical preimage matches the
    // receiving relay's verify exactly: "fed_chat\n{from}\n{channel}\n
    // {content}\n{ts}" (domain-separated so a chat signature can never be
    // replayed as any other message kind). The old code signed
    // "{content}\n{ts}\n{channel}", the second of the three mismatches
    // that kept federation dormant (repaired 2026-08-13).
    let sig_message = format!("fed_chat\n{}\n{}\n{}\n{}", from_name, channel, content, timestamp);
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
        let peer_key = server.public_key.clone();

        tokio::spawn(async move {
            federation_connect_loop(state_clone, server_id, server_name, trust_tier, ws_url, peer_key).await;
        });
        count += 1;
    }
    count
}

/// Connect to a single federated server with exponential backoff reconnection.
///
/// `peer_key` is the peer's pinned server public key from the DB row (set by
/// the /server-add discovery fetch or /server-add-key). It is the peer's
/// IDENTITY: the connection registers under it and every received message's
/// source check compares against it. The row's `server_id` (typically the
/// URL) stays the DB bookkeeping key only. Before this split (2026-08-14,
/// caught by the operator's first live cross-post), outbound sockets
/// registered under the URL while messages identify by key, so the source
/// check dropped every message arriving on an outbound socket: the exact
/// asymmetry the two-relay test's reverse-leg now pins.
pub async fn federation_connect_loop(
    state: Arc<RelayState>,
    server_id: String,
    server_name: String,
    trust_tier: i32,
    ws_url: String,
    peer_key: Option<String>,
) {
    // Identity for the connection map + message source checks. Falls back
    // to the row id when no key is pinned yet (discovery unreachable): the
    // peer's messages then fail the source check until a key is pinned,
    // which is the correct fail-closed behavior for an unverified peer.
    let peer_id = peer_key.unwrap_or_else(|| server_id.clone());
    let mut backoff_secs = 5u64;
    loop {
        tracing::info!("Federation: connecting to {} ({})", server_name, ws_url);
        match tokio_tungstenite::connect_async(&ws_url).await {
            Ok((ws_stream, _)) => {
                backoff_secs = 5; // Reset on successful connect.
                tracing::info!("Federation: connected to {}", server_name);

                let (mut write, mut read) = ws_stream.split();
                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

                // Register the connection under the peer's IDENTITY (key),
                // which also naturally dedupes with an inbound connection
                // from the same peer (same map key).
                {
                    let mut conns = state.federation_connections.write().await;
                    conns.insert(peer_id.clone(), FederatedConnection {
                        tx: tx.clone(),
                        server_id: peer_id.clone(),
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
                // Domain-separated canonical preimage, matching what the
                // receiving relay verifies: "fed_hello\n{server_id}\n{ts}".
                // The old code signed just "{ts}" while the verifier checked
                // "{ts}\n{ts}", so every hello ever sent was rejected: one of
                // the three mismatches that kept federation dormant
                // (repaired 2026-08-13).
                let sig = sign_with_server_key(
                    &state.db,
                    &format!("fed_hello\n{}\n{}", our_pk, timestamp),
                )
                .unwrap_or_default();

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

                // Spawn write pump, with a 30 s WS Ping keepalive: an idle
                // federated link otherwise carries no traffic for hours,
                // and a NAT/middlebox that silently drops the mapping
                // leaves a socket that LOOKS open but delivers nothing.
                // The peer's WS stack auto-answers Pong, which feeds the
                // read pump's liveness timeout below.
                let write_task = tokio::spawn(async move {
                    use tokio_tungstenite::tungstenite::Message as TMessage;
                    let mut ping = tokio::time::interval(tokio::time::Duration::from_secs(30));
                    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                    loop {
                        tokio::select! {
                            msg = rx.recv() => match msg {
                                Some(m) => {
                                    if write.send(TMessage::Text(m.into())).await.is_err() {
                                        break;
                                    }
                                }
                                None => break,
                            },
                            _ = ping.tick() => {
                                if write.send(TMessage::Ping(Vec::new().into())).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                });

                // Read pump: handle incoming messages from federated server.
                // The source identity handed to handle_peer_message is the
                // peer's KEY (peer_id), never the dialed URL. A peer silent
                // for 90 s (3 missed ping rounds) is treated as DEAD: break
                // out so the loop reconnects instead of trusting a rotted
                // NAT mapping forever.
                let state_for_read = state.clone();
                let _sid_for_read = peer_id.clone();
                let read_task = tokio::spawn(async move {
                    use tokio_tungstenite::tungstenite::Message as TMessage;
                    loop {
                        match tokio::time::timeout(
                            tokio::time::Duration::from_secs(90),
                            read.next(),
                        )
                        .await
                        {
                            Err(_) => {
                                tracing::warn!(
                                    "Federation: peer {} silent for 90s, dropping the link to reconnect",
                                    _sid_for_read
                                );
                                break;
                            }
                            Ok(None) | Ok(Some(Err(_))) => break,
                            Ok(Some(Ok(msg))) => {
                                if let TMessage::Text(text) = msg {
                                    if let Ok(relay_msg) =
                                        serde_json::from_str::<RelayMessage>(&text)
                                    {
                                        // One shared handler for both federation
                                        // directions; see handle_peer_message.
                                        handle_peer_message(&state_for_read, &_sid_for_read, relay_msg).await;
                                    }
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

                // Clean up. The conn map is keyed by peer IDENTITY; the DB
                // status row keeps its own id (typically the URL).
                {
                    let mut conns = state.federation_connections.write().await;
                    conns.remove(&peer_id);
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
/// Inbound federation peer lifecycle (2026-08-13 repair). A peer relay
/// dials our /ws and sends FederationHello as its FIRST message; the
/// identify gate hands the whole socket here and never treats it as a
/// user. Order of operations: verify the hello (freshness + operator-
/// granted trust tier + pinned-key signature), reply FederationWelcome ON
/// THIS SOCKET (the old path sent it to local chat clients, so no
/// handshake ever completed), register the connection so our own outbound
/// forwards reach this peer, then pump messages through the same
/// handle_peer_message the outbound direction uses. An unverified hello
/// closes the socket without a reply: unknown callers learn nothing.
pub async fn run_inbound_peer(
    state: Arc<RelayState>,
    mut ws_tx: futures::stream::SplitSink<axum::extract::ws::WebSocket, axum::extract::ws::Message>,
    mut ws_rx: futures::stream::SplitStream<axum::extract::ws::WebSocket>,
    server_id: String,
    public_key: String,
    name: String,
    version: String,
    timestamp: u64,
    signature: String,
) {
    use axum::extract::ws::Message as AxMessage;
    let Some(welcome) = crate::relay::handlers::msg_handlers::handle_federation_hello(
        &state,
        server_id.clone(),
        public_key,
        name.clone(),
        version,
        timestamp,
        signature,
    )
    .await
    else {
        let _ = ws_tx.close().await;
        return;
    };
    if let Ok(json) = serde_json::to_string(&welcome) {
        if ws_tx.send(AxMessage::Text(json.into())).await.is_err() {
            return;
        }
    }
    // The hello only verifies for an operator-added row; read its real
    // trust tier for the registration entry.
    let trust_tier = state
        .db
        .list_federated_servers()
        .ok()
        .and_then(|list| {
            list.into_iter()
                .find(|s| s.server_id == server_id || s.url == server_id)
                .map(|s| s.trust_tier)
        })
        .unwrap_or(2);
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    {
        let mut conns = state.federation_connections.write().await;
        conns.insert(
            server_id.clone(),
            FederatedConnection {
                tx: tx.clone(),
                server_id: server_id.clone(),
                server_name: name.clone(),
                trust_tier,
                connected_at: Instant::now(),
            },
        );
    }
    broadcast_federation_status(&state).await;
    tracing::info!("Federation: inbound peer {} ({}) connected", name, server_id);

    // Write pump: everything forward_to_federation / gossip queues for this
    // peer flows out over the socket THEY opened.
    let write_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_tx.send(AxMessage::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });
    // Read pump: same shared handler as the outbound direction. The DIALER
    // side pings every 30 s (federation_connect_loop), so a healthy link is
    // never silent longer than that; 120 s of silence here means the peer
    // (or its NAT path) is gone -- close so the row shows offline instead
    // of a zombie "connected" that delivers nothing.
    let state_for_read = state.clone();
    let sid_for_read = server_id.clone();
    let read_task = tokio::spawn(async move {
        loop {
            match tokio::time::timeout(tokio::time::Duration::from_secs(120), ws_rx.next()).await {
                Err(_) => {
                    tracing::warn!(
                        "Federation: inbound peer {} silent for 120s, closing",
                        sid_for_read
                    );
                    break;
                }
                Ok(None) | Ok(Some(Err(_))) => break,
                Ok(Some(Ok(msg))) => {
                    if let AxMessage::Text(text) = msg {
                        if let Ok(relay_msg) = serde_json::from_str::<RelayMessage>(&text) {
                            handle_peer_message(&state_for_read, &sid_for_read, relay_msg).await;
                        }
                    }
                }
            }
        }
    });
    tokio::select! {
        _ = write_task => {}
        _ = read_task => {}
    }
    {
        let mut conns = state.federation_connections.write().await;
        conns.remove(&server_id);
    }
    let _ = state.db.update_federated_server_status(&server_id, "offline");
    broadcast_federation_status(&state).await;
    tracing::info!("Federation: inbound peer {} disconnected", server_id);
}

/// Handle ONE message from an authenticated federation peer. Shared by the
/// OUTBOUND read pump (we dialed them) and the INBOUND peer loop (they
/// dialed us; see run_inbound_peer), so the two directions can never drift:
/// factored 2026-08-13 during the federation repair, which found the
/// outbound pump persisting federated chat WITHOUT signature verification
/// while the verifying handler sat unreachable on the user-socket path.
/// `source_server_id` is the identity the SOCKET authenticated as; messages
/// claiming to be from a different server are dropped before any handler
/// runs, so one compromised peer cannot speak in another peer's name.
pub async fn handle_peer_message(
    state: &Arc<RelayState>,
    source_server_id: &str,
    relay_msg: RelayMessage,
) {
    match relay_msg {
        RelayMessage::FederatedChat { server_id, server_name, from_name, content, timestamp, channel, signature } => {
            if server_id != source_server_id {
                tracing::warn!(
                    "Federation: peer {} sent chat claiming server_id {} — dropped",
                    source_server_id, server_id
                );
                return;
            }
            // Full verified path: freshness + trust tier + signature over the
            // canonical preimage, then persist + broadcast.
            crate::relay::handlers::msg_handlers::handle_federated_chat(
                state, server_id, server_name, from_name, content, timestamp, channel, signature,
            )
            .await;
        }
        RelayMessage::FederationWelcome { server_id, name, channels } => {
            tracing::info!("Federation: welcome from {} — channels: {:?}", name, channels);
            let _ = state.db.update_federated_server_status(&server_id, "online");
        }
        // Profile gossip from a federated peer — cache if it verifies.
        RelayMessage::ProfileGossip { public_key, name, bio, avatar_url, banner_url, socials, pronouns, location, website, timestamp, signature } => {
            // The Dilithium3 profile-signature verify inside
            // should_accept_profile_gossip is CPU-bound; run it OFF the
            // async worker pool so a gossip burst can't starve tokio.
            // A panicked/cancelled verify task fails closed (reject).
            let accept = {
                let pk = public_key.clone();
                let nm = name.clone();
                let bo = bio.clone();
                let av = avatar_url.clone();
                let bn = banner_url.clone();
                let so = socials.clone();
                let pr = pronouns.clone();
                let lo = location.clone();
                let we = website.clone();
                let sg = signature.clone();
                tokio::task::spawn_blocking(move || {
                    should_accept_profile_gossip(
                        &pk, &nm, &bo, &av, &bn,
                        &so, &pr, &lo, &we,
                        timestamp, &sg,
                    )
                })
                .await
                .unwrap_or(false)
            };
            if !accept {
                tracing::warn!("Federation: rejecting profile gossip for {} — signature did not verify", &name);
                return;
            }
            tracing::debug!("Federation: received profile gossip for {}", &name);
            let _ = state.db.store_signed_profile(
                &public_key, &name, &bio, &avatar_url, &banner_url,
                &socials, &pronouns, &location, &website, timestamp, &signature,
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
                        source_server_id
                    );
                    return;
                }
            };
            let payload = match B64.decode(&payload_b64) {
                Ok(b) => b,
                Err(_) => return,
            };
            let signature = match B64.decode(&signature_b64) {
                Ok(b) => b,
                Err(_) => return,
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

            // Per-SOURCE inbound rate limit (audit 2026-06-12): one peer
            // sending valid objects at line rate would otherwise be
            // AMPLIFIED N-fold by re-gossip to every other peer.
            {
                const INBOUND_MAX_PER_SEC: usize = 50;
                let allow = {
                    let mut rate = state.federation_rate.lock().unwrap();
                    const FED_RATE_MAP_CAP: usize = 10_000;
                    if rate.len() > FED_RATE_MAP_CAP {
                        let cutoff = Instant::now();
                        rate.retain(|_, times| {
                            times.retain(|t| cutoff.duration_since(*t).as_secs() < 300);
                            !times.is_empty()
                        });
                    }
                    let times = rate
                        .entry(format!("{}:inbound", source_server_id))
                        .or_default();
                    let now = Instant::now();
                    times.retain(|t| now.duration_since(*t).as_secs() < 1);
                    if times.len() < INBOUND_MAX_PER_SEC {
                        times.push(now);
                        true
                    } else {
                        false
                    }
                };
                if !allow {
                    tracing::warn!(
                        "Federation: inbound gossip rate limit hit for source {} — dropping object {}",
                        source_server_id, object_id
                    );
                    return;
                }
            }

            // put_signed_object verifies the Dilithium3 signature.
            match state.db.put_signed_object(&object, Some(source_server_id)) {
                Ok(true) => {
                    tracing::debug!(
                        "Federation: accepted {} object {} from {}",
                        object_type, object_id, source_server_id
                    );
                    // Phase 3 PR 2: multi-hop gossip — re-emit to peers OTHER
                    // than the source. INSERT OR IGNORE breaks cycles.
                    let state_for_gossip = state.clone();
                    let object_for_gossip = object.clone();
                    let exclude = source_server_id.to_string();
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
                        object_type, source_server_id, e
                    );
                }
            }
        }
        _ => {} // Ignore other message types from federation peers.
    }
}

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
/// client-supplied **Dilithium3** signature (hex) over `canonical_profile_message(...)`;
/// pass an empty string when the client did not sign (peers will then accept
/// under the trust-by-source model — see `should_accept_profile_gossip`).
/// Was Ed25519 pre-cutover; switched in v0.276.0 so the profile signing path
/// uses the same identity key the user actually holds (Dilithium3).
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
    use crate::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};

    // Full-PQ: profile gossip now Dilithium3-signed (was Ed25519). The
    // tests use a deterministic seed to keep them stable; the verifier
    // path under test is the same one production uses
    // (`verify_dilithium` inside `verify_profile_signature`).
    fn fixture() -> (DilithiumKeypair, String) {
        let seed = [7u8; 32];
        let dil_seed = derive_dilithium_seed(&seed);
        let kp = DilithiumKeypair::from_seed(&dil_seed);
        let pk_hex = hex::encode(kp.public_key());
        (kp, pk_hex)
    }

    fn sign_profile(kp: &DilithiumKeypair, msg: &str) -> String {
        hex::encode(kp.sign(msg.as_bytes()))
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
        let other_dil_seed = derive_dilithium_seed(&[9u8; 32]);
        let other_sk = DilithiumKeypair::from_seed(&other_dil_seed);
        let other_pk_hex = hex::encode(other_sk.public_key());

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
    fn accept_rejects_empty_signature() {
        // The old trust-by-source model ADMITTED empty signatures, which
        // made an empty string a skeleton key over every cached profile.
        // Closed in the 2026-08-13 federation repair: no signature, no
        // cache write, no exceptions.
        let (_sk, pk_hex) = fixture();
        assert!(!should_accept_profile_gossip(
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
