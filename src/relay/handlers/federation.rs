//! Server-to-server federation functions extracted from relay.rs.
//! Handles outbound WebSocket connections to peer servers and message forwarding.

use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use futures::{SinkExt, StreamExt};

use crate::relay::core::object::Object;
use crate::relay::relay::{FederatedConnection, FederationServerStatus, RelayMessage, RelayState};
use crate::relay::storage::Storage;

/// Sign a message with the server's Dilithium3 key.
///
/// Was Ed25519 until 2026-09-06. The stored value is a 32-byte SEED, expanded
/// here the same way a user's identity is, so the signing key never sits in the
/// database in expanded form.
pub fn sign_with_server_key(db: &Storage, message: &str) -> Option<String> {
    use crate::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};
    let (_, seed_hex) = db.get_or_create_server_keypair().ok()?;
    let seed = hex::decode(&seed_hex).ok()?;
    if seed.len() != 32 {
        return None;
    }
    let kp = DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed));
    Some(hex::encode(kp.sign(message.as_bytes())))
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

/// How often a live federation link checks that its peer is still in the registry.
pub(crate) const REGISTRY_CHECK_SECS: u64 = 15;

/// Whether the federation registry still lists `id` (as its row id, its URL or its pinned key).
/// None when the registry could not be read: that is not a removal.
pub(crate) fn registry_lists(state: &RelayState, id: &str) -> Option<bool> {
    let rows = state.db.list_federated_servers().ok()?;
    Some(rows.iter().any(|s| s.server_id == id || s.url == id || s.public_key.as_deref() == Some(id)))
}

/// Resolves once the registry no longer lists `id`, checked every `every`, so a live link to a peer
/// an admin removed closes (2026-10-11; before, removal only stopped the next reconnect).
pub(crate) async fn until_removed(state: Arc<RelayState>, id: String, every: Duration) {
    loop {
        tokio::time::sleep(every).await;
        if registry_lists(&state, &id) == Some(false) {
            return;
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
    // The pin is read fresh on every attempt below (2026-10-11), so a key re-checked with
    // `recheck_peer_key` is used from the next connection on, without a restart; the one passed in
    // only names the peer until then.
    let _ = peer_key;
    let mut backoff_secs = 5u64;
    let mut quick_drops = 0u32;
    loop {
        let rows = match state.db.list_federated_servers() {
            Ok(rows) => rows,
            Err(e) => {
                // Not a removal: the registry could not be read. Try again shortly.
                tracing::warn!("Federation: could not read the registry for {} ({e}); trying again", server_name);
                tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs)).await;
                continue;
            }
        };
        let Some(row) = rows.into_iter().find(|s| s.server_id == server_id) else {
            tracing::info!("Federation: {} is no longer in the registry, so its connection stops", server_name);
            return;
        };
        let peer_id = row.public_key.clone().unwrap_or_else(|| server_id.clone());
        // How long the link held, for the wait before the next try (None: it never connected).
        let mut lasted: Option<u64> = None;
        tracing::info!("Federation: connecting to {} ({})", server_name, ws_url);
        match tokio_tungstenite::connect_async(&ws_url).await {
            Ok((ws_stream, _)) => {
                let linked_at = Instant::now();
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

                // Wait for either to finish (connection dropped), or for an admin to remove this
                // peer, which closes a live link within REGISTRY_CHECK_SECS (until 2026-10-11 it
                // lived on until it dropped by itself, still carrying chat). Both pumps stop.
                let mut write_task = write_task;
                let mut read_task = read_task;
                let removed = tokio::select! {
                    _ = &mut write_task => false,
                    _ = &mut read_task => false,
                    _ = until_removed(state.clone(), server_id.clone(), Duration::from_secs(REGISTRY_CHECK_SECS)) => true,
                };
                write_task.abort();
                read_task.abort();
                if removed {
                    tracing::info!("Federation: {} was removed from the registry, so its link closes", server_name);
                }
                lasted = Some(linked_at.elapsed().as_secs());

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

        // A link closed within a minute of the hello counts as refused. Said once per streak, in
        // the words that fix it: the usual cause is a peer that does not trust this server's
        // current key (public.guide refused every hello from 2026-09-06, when server keys moved
        // to Dilithium3, until 2026-10-11, and was dialled every 6 s the whole time).
        match lasted {
            Some(s) if s < STABLE_LINK_SECS => {
                quick_drops += 1;
                if quick_drops == QUICK_DROPS_TO_SAY {
                    tracing::warn!(
                        "Federation: {} has closed the link {} times within a minute of our hello. It may not trust this server's current key: on {}, re-check this server's key (Server Settings > Federation, Re-check key, or /server-recheck). Waiting longer between tries until a link holds.",
                        server_name,
                        quick_drops,
                        server_name
                    );
                }
            }
            Some(_) => quick_drops = 0,
            None => {}
        }
        backoff_secs = next_backoff(backoff_secs, lasted);
        tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs)).await;
    }
}

/// A link that held this long counts as working, and the waits between tries start again from
/// 5 seconds.
pub const STABLE_LINK_SECS: u64 = 60;
/// After this many links in a row closed within a minute, the log says why it may be happening.
pub const QUICK_DROPS_TO_SAY: u32 = 3;

/// The wait before the next connection attempt, in seconds, after a link that held for
/// `lasted_secs` (None when it never connected). A link that held a minute starts the waits again
/// from 5 s; one that never connected or closed sooner waits twice as long as the last time, up
/// to 5 minutes. Before 2026-10-11 any connect reset the wait to 5 s, so a peer that closed every
/// link right after the hello was dialled every 6 s forever (about 14,000 log lines a day).
pub fn next_backoff(prev_secs: u64, lasted_secs: Option<u64>) -> u64 {
    match lasted_secs {
        Some(s) if s >= STABLE_LINK_SECS => 5,
        _ => (prev_secs.max(5) * 2).min(300),
    }
}

/// The length of a server key in hex: a Dilithium3 (ML-DSA-65) public key is 1,952 bytes. Server
/// identity moved to it on 2026-09-06; a 64-hex Ed25519 key is no longer a server key.
pub const SERVER_KEY_HEX_LEN: usize = 3904;

/// Whether `key` is a server key in the form this relay pins: lower-case hex of
/// [`SERVER_KEY_HEX_LEN`] characters.
pub fn is_server_key(key: &str) -> bool {
    key.len() == SERVER_KEY_HEX_LEN && key.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// What re-checking a peer's key found.
#[derive(Debug, Clone, PartialEq)]
pub struct Rechecked {
    /// The peer's name, as its server-info gives it.
    pub name: String,
    /// Whether the pinned key changed.
    pub changed: bool,
}

/// Re-read a peer's key from its own `/api/server-info`, at the address the operator added it by,
/// and pin it: the same discovery an add does, for when a peer's key has changed (as every
/// server's did on 2026-09-06). The connection loop reads the pin fresh on its next try. A peer
/// added by key alone has no address to ask, and an answer that is not a current server key (an
/// old version still on Ed25519) changes nothing. Its words say what to do.
pub async fn recheck_peer_key(state: &Arc<RelayState>, server_id: &str) -> Result<Rechecked, String> {
    let row = state
        .db
        .list_federated_servers()
        .map_err(|e| format!("Database error: {e}"))?
        .into_iter()
        .find(|s| s.server_id == server_id)
        .ok_or_else(|| "No such server in the registry.".to_string())?;
    if !(row.url.starts_with("https://") || row.url.starts_with("http://")) {
        return Err(format!(
            "{} was added by its key, not by a web address, so there is nowhere to fetch a new key from. Remove it and add it again with its new key (from its Host Node page).",
            row.name
        ));
    }
    let info_url = format!("{}/api/server-info", row.url.trim_end_matches('/'));
    let resp = state
        .http_client
        .get(&info_url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("Could not reach {}: {e}", row.name))?;
    if !resp.status().is_success() {
        return Err(format!("{} answered {} for its server information.", row.name, resp.status()));
    }
    let info: serde_json::Value =
        resp.json().await.map_err(|e| format!("{} sent server information that could not be read: {e}", row.name))?;
    let key = info["public_key"].as_str().unwrap_or("").to_ascii_lowercase();
    if !is_server_key(&key) {
        return Err(format!(
            "{} did not give a current server key ({} hex characters, post-quantum). It may run a version from before 2026-09-06: update it first.",
            row.name, SERVER_KEY_HEX_LEN
        ));
    }
    let changed = row.public_key.as_deref() != Some(key.as_str());
    let name = info["name"].as_str().filter(|n| !n.trim().is_empty()).unwrap_or(&row.name).to_string();
    let accord = info["accord_compliant"].as_bool().unwrap_or(row.accord_compliant);
    state
        .db
        .update_federated_server_info(server_id, &name, Some(&key), accord)
        .map_err(|e| format!("Database error: {e}"))?;
    if changed {
        tracing::info!("Federation: re-pinned {} ({}) to the key it gives now", name, server_id);
    }
    Ok(Rechecked { name, changed })
}

/// The words a re-check answers with, for the chat command and the admin API alike.
pub fn recheck_words(r: &Result<Rechecked, String>) -> String {
    match r {
        Ok(Rechecked { name, changed: true }) => {
            format!("Re-checked {name}: its key had changed and is pinned now. The next connection uses it.")
        }
        Ok(Rechecked { name, changed: false }) => format!("Re-checked {name}: its key is the one already pinned."),
        Err(words) => words.clone(),
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
    // Until either pump ends or an admin removes this peer (the link then closes within
    // REGISTRY_CHECK_SECS instead of living on, 2026-10-11).
    let mut write_task = write_task;
    let mut read_task = read_task;
    let removed = tokio::select! {
        _ = &mut write_task => false,
        _ = &mut read_task => false,
        _ = until_removed(state.clone(), server_id.clone(), Duration::from_secs(REGISTRY_CHECK_SECS)) => true,
    };
    write_task.abort();
    read_task.abort();
    if removed {
        tracing::info!("Federation: inbound peer {} was removed from the registry, so its link closes", server_id);
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

#[cfg(test)]
mod server_identity_tests {
    use super::*;
    use crate::relay::handlers::broadcast::{verify_dilithium_signature, verify_ed25519_signature};
    use crate::relay::storage::Storage;

    fn test_db() -> Storage {
        Storage::open_temp("srvid")
    }

    /// The round trip a federation hello actually performs: this server signs
    /// "fed_hello\n{id}" and a peer verifies it against the pinned public key.
    ///
    /// Server identity was Ed25519 until 2026-09-06 while every user identity
    /// around it had been ML-DSA-65 since the v0.264 cutover, which left the
    /// federation handshake, federated chat and every server announcement
    /// authenticated by the one key a quantum adversary could forge.
    #[test]
    fn a_server_signature_verifies_under_dilithium() {
        let db = test_db();
        let (pk_hex, seed_hex) = db.get_or_create_server_keypair().unwrap();

        // A Dilithium3 public key is 1952 bytes; the stored secret is a 32-byte
        // SEED, not the expanded key, so the row stays small.
        assert_eq!(pk_hex.len(), 1952 * 2, "server key must be Dilithium3");
        assert_eq!(seed_hex.len(), 64, "the stored secret must be a 32-byte seed");

        let ts = 1_751_328_000_000u64;
        let preimage = "fed_hello\ntest-server";
        let sig = sign_with_server_key(&db, &format!("{preimage}\n{ts}")).expect("signed");

        assert!(
            verify_dilithium_signature(&pk_hex, preimage, ts, &sig),
            "a peer must be able to verify this server's hello"
        );
        // Tamper with the preimage: must fail.
        assert!(
            !verify_dilithium_signature(&pk_hex, "fed_hello\nsomeone-else", ts, &sig),
            "a signature must not verify for a different server id"
        );
        // And the OLD verifier must not accept it, which is the whole point of
        // the break: a peer still running the Ed25519 check cannot be fooled
        // into accepting a PQ signature, it simply refuses and must re-pin.
        assert!(
            !verify_ed25519_signature(&pk_hex, preimage, ts, &sig),
            "the Ed25519 path must reject, not silently accept"
        );
    }

    /// The identity is stable across calls, because it is stored and re-derived
    /// from the seed rather than regenerated.
    #[test]
    fn the_server_identity_is_stable_and_has_a_did() {
        let db = test_db();
        let (pk1, seed1) = db.get_or_create_server_keypair().unwrap();
        let (pk2, seed2) = db.get_or_create_server_keypair().unwrap();
        assert_eq!(pk1, pk2, "the server must not mint a new identity per call");
        assert_eq!(seed1, seed2);

        let did = db.server_did().unwrap();
        assert!(did.starts_with("did:hum:"), "got {did}");
        // Short enough to print on a card, unlike the 3904-char key.
        assert!(did.len() < 40, "a server DID must be short, got {} chars", did.len());
        // And it is the SAME derivation a person's DID uses.
        let pk = hex::decode(&pk1).unwrap();
        assert_eq!(did, crate::relay::core::did::did_for_pubkey(&pk));
    }
}

#[cfg(test)]
mod recheck_tests {
    use super::*;
    use crate::relay::relay::RelayState;
    use crate::relay::storage::Storage;

    /// THE WAIT BETWEEN TRIES BACKS OFF WHEN A PEER KEEPS CLOSING THE LINK: a link that held a
    /// minute starts the waits again from 5 s; one closed sooner, or never made, waits twice as long
    /// as the last time, up to 5 minutes. (public.guide closed every link within a second from
    /// 2026-09-06 to 2026-10-11 and was dialled every 6 s, about 14,000 log lines a day.)
    ///
    /// Seen red 2026-10-11 with a quick drop answered like a held link (the old reset on connect):
    /// "a link closed after 1 s waits longer / left: 5 / right: 20".
    #[test]
    fn the_wait_backs_off_when_a_peer_keeps_closing_the_link() {
        assert_eq!(next_backoff(10, Some(1)), 20, "a link closed after 1 s waits longer");
        assert_eq!(next_backoff(160, Some(0)), 300, "up to 5 minutes");
        assert_eq!(next_backoff(300, None), 300, "a connect that failed waits the most too");
        assert_eq!(next_backoff(300, Some(STABLE_LINK_SECS)), 5, "a link that held a minute starts again from 5 s");
    }

    /// A LINK TO A REMOVED PEER CLOSES: `until_removed` waits while the registry lists the peer (by
    /// row id, address or pinned key) and resolves once an admin removes it; a registry that cannot
    /// be read is not a removal. (Until 2026-10-11 removal only stopped the next reconnect, and a
    /// healthy link kept carrying chat.)
    ///
    /// Seen red 2026-10-11 with `registry_lists` answering true whatever the registry held: the
    /// check that an address it does not list is not listed failed (Some(true)).
    #[tokio::test]
    async fn a_link_to_a_removed_peer_closes() {
        let state = Arc::new(RelayState::new(Storage::open_temp("fed_until_removed")));
        state.db.add_federated_server("peer-row", "Peer", "https://peer.example").unwrap();
        assert_eq!(registry_lists(&state, "peer-row"), Some(true));
        assert_eq!(registry_lists(&state, "https://peer.example"), Some(true), "by its address");
        assert_eq!(registry_lists(&state, "someone-else"), Some(false));
        let wait = tokio::spawn(until_removed(state.clone(), "peer-row".to_string(), Duration::from_millis(10)));
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert!(!wait.is_finished(), "a listed peer's link stays");
        state.db.remove_federated_server("peer-row").unwrap();
        tokio::time::timeout(Duration::from_secs(2), wait).await.expect("the wait ends once it is removed").unwrap();
    }

    /// A SERVER KEY IS TODAY'S KEY: 3,904 lower-case hex characters (Dilithium3). A 64-hex Ed25519
    /// key, the only length /server-add-key and the admin add_key accepted until 2026-10-11, is not.
    ///
    /// Seen red 2026-10-11 with the length check at 64: "today's key is a server key".
    #[test]
    fn a_server_key_is_todays_key() {
        assert!(is_server_key(&"a1".repeat(1952)), "today's key is a server key");
        assert!(!is_server_key(&"ab".repeat(32)), "an Ed25519 key is not");
        assert!(!is_server_key(&"A1".repeat(1952)), "upper case is not the pinned form");
    }

    /// A stand-in peer on this machine only, answering /api/server-info with `key`.
    async fn peer_giving(key: String) -> (String, tokio::task::JoinHandle<()>) {
        let app = axum::Router::new().route(
            "/api/server-info",
            axum::routing::get(move || {
                let key = key.clone();
                async move { axum::Json(serde_json::json!({ "name": "Peer", "public_key": key, "accord_compliant": false })) }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        (format!("http://{addr}"), task)
    }

    /// RE-CHECKING A PEER'S KEY PINS THE ONE IT GIVES NOW: a peer whose key changed is re-pinned
    /// from its own server-info (and a second re-check says it is unchanged); a peer still giving
    /// an old 64-hex key is refused with words that say to update it; a peer added by key alone
    /// has no address to ask.
    ///
    /// Seen red 2026-10-11 with the pin update left out of `recheck_peer_key`: "the new key is
    /// pinned" (the old key was still pinned).
    #[tokio::test]
    async fn rechecking_a_peer_pins_the_key_it_gives_now() {
        let state = Arc::new(RelayState::new(Storage::open_temp("fed_recheck")));
        let new_key = "b2".repeat(1952);
        let (url, server) = peer_giving(new_key.clone()).await;
        state.db.add_federated_server(&url, "Peer", &url).unwrap();
        state.db.update_federated_server_info(&url, "Peer", Some(&"a1".repeat(1952)), false).unwrap();

        let first = recheck_peer_key(&state, &url).await;
        assert_eq!(first, Ok(Rechecked { name: "Peer".into(), changed: true }), "{}", recheck_words(&first));
        let pinned = state.db.list_federated_servers().unwrap().into_iter().find(|s| s.server_id == url).unwrap().public_key;
        assert!(pinned.as_deref() == Some(new_key.as_str()), "the new key is pinned");
        let again = recheck_peer_key(&state, &url).await;
        assert_eq!(again, Ok(Rechecked { name: "Peer".into(), changed: false }));
        assert!(recheck_words(&again).contains("already pinned"));
        server.abort();

        let (old_url, old_server) = peer_giving("ab".repeat(32)).await;
        state.db.add_federated_server(&old_url, "Old", &old_url).unwrap();
        let old = recheck_peer_key(&state, &old_url).await.unwrap_err();
        assert!(old.contains("before 2026-09-06"), "{old}");
        old_server.abort();

        let key = "c3".repeat(1952);
        state.db.add_federated_server(&key, "Home node", "outbound-only").unwrap();
        let by_key = recheck_peer_key(&state, &key).await.unwrap_err();
        assert!(by_key.contains("added by its key"), "{by_key}");
        assert_eq!(recheck_peer_key(&state, "nobody").await.unwrap_err(), "No such server in the registry.");
    }
}
