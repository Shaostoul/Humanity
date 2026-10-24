//! Server-to-server federation functions extracted from relay.rs.
//! Handles outbound WebSocket connections to peer servers and message forwarding.

use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use futures::{SinkExt, StreamExt};

use crate::relay::core::object::Object;
use crate::relay::core::profile_copy::{self, PublicCopy};
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

/// Decide whether to keep an inbound `ProfileGossip` copy (BUG-191): it must carry a signature
/// of the right shape, keep the save's own limits (`profile_copy::within_limits`: what a save
/// would refuse is never kept, and no line break outside the bio, so the signed words split one
/// way only), and verify as its owner's Dilithium3 signature over `profile_copy::copy_text`.
/// Profiles are self-certifying, so which peer relayed a copy never has to be trusted for its
/// content. Before BUG-191 no app signed a profile, so every copy was refused here; each person's
/// own app signs from this release. CPU-bound: run it off the async pool.
pub fn should_accept_profile_gossip(public_key_hex: &str, copy: &PublicCopy, timestamp: u64, signature_hex: &str) -> bool {
    // No signature, no cache write. The old fail-open here meant an empty signature was a
    // skeleton key: any message shaped like gossip could overwrite any cached profile (closed
    // 2026-08-13, federation repair).
    if signature_hex.is_empty() {
        return false;
    }
    profile_copy::within_limits(public_key_hex, copy) && profile_copy::verify_copy(public_key_hex, copy, timestamp, signature_hex)
}

/// The words a server signs for a chat line it sends its links, and a receiver checks: domain
/// separated ("fed_chat") so a chat signature can never pass as any other kind, and naming the
/// author's KEY (since 2026-10-22: a receiver refuses the words of a key it, or a server sharing
/// the room, has banned, so the key has to be what the origin vouches for, not just the name).
/// The verify helper appends the time the same way.
pub fn chat_words(from_key: &str, from_name: &str, channel: &str, content: &str, timestamp: u64) -> String {
    format!("fed_chat\n{from_key}\n{from_name}\n{channel}\n{content}\n{timestamp}")
}

/// Send a chat line posted here in a shared room to the linked servers: signed, numbered in the
/// outbox (federation_log.rs), so a link that is down or paused gets it when it catches up.
/// Before 2026-10-22 it went live only, and a link over 10 a second simply lost the rest.
pub async fn forward_to_federation(state: &Arc<RelayState>, channel: &str, from_key: &str, from_name: &str, content: &str, timestamp: u64) {
    let (server_id, _) = match state.db.get_or_create_server_keypair() {
        Ok(kp) => kp,
        Err(_) => return,
    };
    let server_name = std::env::var("SERVER_NAME").unwrap_or_else(|_| "Humanity Relay".to_string());
    let signature = sign_with_server_key(&state.db, &chat_words(from_key, from_name, channel, content, timestamp));
    let federated_msg = RelayMessage::FederatedChat {
        server_id,
        server_name,
        from_name: from_name.to_string(),
        content: content.to_string(),
        timestamp,
        channel: channel.to_string(),
        signature,
        from_key: Some(from_key.to_string()),
    };
    let Ok(event) = serde_json::to_value(&federated_msg) else { return };
    crate::relay::handlers::federation_log::publish(state, "chat", from_key, timestamp, "", &event).await;
}

/// A server key as the log shows it: its first 16 hex characters. A whole Dilithium3 key is 3,904
/// characters, and every hello and connect wrote it out in full (2026-10-12).
pub(crate) fn key_in_log(key: &str) -> String {
    if key.len() <= 16 { key.to_string() } else { format!("{}...", &key[..16]) }
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
                // Its place in our numbered log, and ours in its log (federation_log.rs). The
                // request is queued behind the hello written below.
                let queued = crate::relay::handlers::federation_log::link_up(&state, &peer_id, &tx).await;

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
                                    crate::relay::handlers::federation_log::written(&queued);
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
                let tx_for_read = tx.clone();
                let read_task = tokio::spawn(async move {
                    use tokio_tungstenite::tungstenite::Message as TMessage;
                    let silent = Duration::from_secs(90);
                    let mut heard = Instant::now();
                    loop {
                        // Woken sooner when a pause on this peer ends, to ask for its catch-up.
                        let wait = crate::relay::handlers::federation_log::read_wait(&state_for_read, &_sid_for_read, silent.saturating_sub(heard.elapsed()));
                        match tokio::time::timeout(wait, read.next()).await {
                            Err(_) => {
                                crate::relay::handlers::federation_log::tick(&state_for_read, &_sid_for_read, &tx_for_read).await;
                                if heard.elapsed() >= silent {
                                    tracing::warn!(
                                        "Federation: peer {} silent for 90s, dropping the link to reconnect",
                                        key_in_log(&_sid_for_read)
                                    );
                                    break;
                                }
                            }
                            Ok(None) | Ok(Some(Err(_))) => break,
                            Ok(Some(Ok(msg))) => {
                                heard = Instant::now();
                                if let TMessage::Text(text) = msg {
                                    // One shared path for both federation directions: the flow
                                    // limits, the numbered log, then handle_peer_message.
                                    crate::relay::handlers::federation_log::ingest(&state_for_read, &_sid_for_read, &tx_for_read, &text).await;
                                    heard = Instant::now();
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
                crate::relay::handlers::federation_log::link_down(&state, &peer_id);
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
    tracing::info!("Federation: inbound peer {} ({}) connected", name, key_in_log(&server_id));
    // Its place in our numbered log, and ours in its log (federation_log.rs): our request goes
    // after the welcome already on the socket.
    let queued = crate::relay::handlers::federation_log::link_up(&state, &server_id, &tx).await;
    // The catch-up (BUG-191 plan 3.7): queued now, sent by the write pump below once it starts,
    // after the welcome already on the socket. The dialer sends its own on our welcome.
    send_own_copies(&state, &server_id).await;

    // Write pump: everything forward_to_federation / gossip queues for this
    // peer flows out over the socket THEY opened.
    let write_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_tx.send(AxMessage::Text(msg.into())).await.is_err() {
                break;
            }
            crate::relay::handlers::federation_log::written(&queued);
        }
    });
    // Read pump: same shared handler as the outbound direction. The DIALER
    // side pings every 30 s (federation_connect_loop), so a healthy link is
    // never silent longer than that; 120 s of silence here means the peer
    // (or its NAT path) is gone -- close so the row shows offline instead
    // of a zombie "connected" that delivers nothing.
    let state_for_read = state.clone();
    let sid_for_read = server_id.clone();
    let tx_for_read = tx.clone();
    let read_task = tokio::spawn(async move {
        let silent = Duration::from_secs(120);
        let mut heard = Instant::now();
        loop {
            // Woken sooner when a pause on this peer ends, to ask for its catch-up.
            let wait = crate::relay::handlers::federation_log::read_wait(&state_for_read, &sid_for_read, silent.saturating_sub(heard.elapsed()));
            match tokio::time::timeout(wait, ws_rx.next()).await {
                Err(_) => {
                    crate::relay::handlers::federation_log::tick(&state_for_read, &sid_for_read, &tx_for_read).await;
                    if heard.elapsed() >= silent {
                        tracing::warn!(
                            "Federation: inbound peer {} silent for 120s, closing",
                            key_in_log(&sid_for_read)
                        );
                        break;
                    }
                }
                Ok(None) | Ok(Some(Err(_))) => break,
                Ok(Some(Ok(msg))) => {
                    heard = Instant::now();
                    if let AxMessage::Text(text) = msg {
                        crate::relay::handlers::federation_log::ingest(&state_for_read, &sid_for_read, &tx_for_read, &text).await;
                        heard = Instant::now();
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
        tracing::info!("Federation: inbound peer {} was removed from the registry, so its link closes", key_in_log(&server_id));
    }
    {
        let mut conns = state.federation_connections.write().await;
        conns.remove(&server_id);
    }
    crate::relay::handlers::federation_log::link_down(&state, &server_id);
    let _ = state.db.update_federated_server_status(&server_id, "offline");
    broadcast_federation_status(&state).await;
    tracing::info!("Federation: inbound peer {} disconnected", key_in_log(&server_id));
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
        RelayMessage::FederatedChat { server_id, server_name, from_name, content, timestamp, channel, signature, from_key } => {
            if server_id != source_server_id {
                tracing::warn!(
                    "Federation: peer {} sent chat claiming server_id {} — dropped",
                    key_in_log(source_server_id), key_in_log(&server_id)
                );
                return;
            }
            // Full verified path: trust tier + signature over the canonical words, the author's
            // standing here, then persist + broadcast. It arrives only numbered (federation_log.rs).
            crate::relay::handlers::msg_handlers::handle_federated_chat(
                state, server_id, server_name, from_name, content, timestamp, channel, signature, from_key.unwrap_or_default(),
            )
            .await;
        }
        RelayMessage::FederationWelcome { server_id, name, channels } => {
            tracing::info!("Federation: welcome from {} — channels: {:?}", name, channels);
            let _ = state.db.update_federated_server_status(&server_id, "online");
            // The link we dialed is up: the catch-up (BUG-191 plan 3.7). The side that was dialed
            // sends its own from run_inbound_peer.
            send_own_copies(state, source_server_id).await;
        }
        // A copy of someone's profile from a federated peer (BUG-191): kept if it keeps the
        // limits, verifies as its owner's signature and is newer than the one held. A withdrawal
        // (the person went unlisted on another server) replaces the copy held, as any newer copy
        // does, and nothing else: this server's own profile for them, and every other copy, stay
        // as they are. A withdrawal for a key never held is kept nowhere.
        RelayMessage::ProfileGossip { public_key, name, bio, avatar_url, banner_url, socials, pronouns, location, website, timestamp, signature } => {
            let copy = PublicCopy { name, bio, avatar_url, banner_url, socials, pronouns, location, website };
            // The Dilithium3 verify inside should_accept_profile_gossip is CPU-bound; run it OFF
            // the async worker pool so a gossip burst can't starve tokio. A panicked/cancelled
            // verify task fails closed (refused).
            let accept = {
                let (pk, c, sg) = (public_key.clone(), copy.clone(), signature.clone());
                tokio::task::spawn_blocking(move || should_accept_profile_gossip(&pk, &c, timestamp, &sg))
                    .await
                    .unwrap_or(false)
            };
            if !accept {
                tracing::warn!(
                    "Federation: refused a profile copy for {} from {}: over the limits or the signature did not verify",
                    key_in_log(&public_key),
                    key_in_log(source_server_id)
                );
                return;
            }
            let kept = match state.db.store_signed_profile(&public_key, &copy, timestamp, &signature) {
                Ok(kept) => kept,
                Err(e) => {
                    tracing::error!("Federation: could not keep a profile copy for {}: {e}", key_in_log(&public_key));
                    return;
                }
            };
            if !kept {
                tracing::debug!(
                    "Federation: profile copy for {} from {}: not newer than the one held, or a withdrawal of nothing held",
                    key_in_log(&public_key),
                    key_in_log(source_server_id)
                );
                return;
            }
            // Passing on (plan 3.6): a copy newly kept for one of THIS server's accounts goes on
            // to the other links, never back to the one it came from, the way signed objects are
            // passed on below. With a third server this carries a withdrawal, or a field made
            // friends-only, to every server that has the copy. It cannot loop: a copy that is
            // not newer than the one held is not kept, so it goes no further. A copy of someone
            // with no account here is kept but not passed on: their own server sends it.
            let ours = match state.db.name_for_key(&public_key) {
                Ok(name) => name.is_some(),
                Err(e) => {
                    tracing::error!("Federation: could not look up an account for {}, so its copy is not passed on: {e}", key_in_log(&public_key));
                    false
                }
            };
            tracing::debug!(
                "Federation: kept the profile copy for {} from {}{}",
                key_in_log(&public_key),
                key_in_log(source_server_id),
                if ours { ", one of ours, so it goes on to the other links" } else { "" }
            );
            if ours {
                gossip_profile(state, &public_key, &copy, timestamp, &signature, Some(source_server_id)).await;
            }
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

            // The per-source inbound limit (audit 2026-06-12: one peer sending valid objects at
            // line rate would be AMPLIFIED N-fold by re-gossip) is the object allowance in
            // federation_flow.rs since 2026-10-22: over it the peer is slowed, then paused, and
            // nothing is dropped (the old 50-a-second cap here dropped the rest for good).

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

    let Ok(event) = serde_json::to_value(&msg) else { return };
    // Numbered in the outbox (federation_log.rs) and sent to every link but the one it came from.
    // The old outbound cap of 50 a second dropped the rest; the receiver's allowance paces it now.
    let seq = crate::relay::handlers::federation_log::publish(state, "object", "", 0, exclude_server_id.unwrap_or(""), &event).await;
    tracing::debug!("Federation: gossiped {} object {} as {:?} (excluded={:?})", object.object_type, object_id, seq, exclude_server_id);
}

/// Send the copy of `public_key`'s profile that other servers keep to every linked server but
/// `exclude` (the one a copy came from, when it is passed on; BUG-191 plan 3.6). Called when this
/// server keeps a copy its owner's app signed (handlers/profile.rs `handle_profile_sign`), and for
/// one of this server's own accounts, a copy kept from a peer. `signature` is the owner's
/// Dilithium3 signature (hex) over `profile_copy::copy_text`; an unsigned copy is never sent, since
/// no server would keep it.
pub async fn gossip_profile(
    state: &Arc<RelayState>,
    public_key: &str,
    copy: &PublicCopy,
    timestamp: u64,
    signature: &str,
    exclude: Option<&str>,
) {
    let Some(json) = profile_gossip_text(public_key, copy, timestamp, signature) else {
        return;
    };
    // Numbered in the outbox (federation_log.rs), so a link that is down or paused gets it when it
    // catches up; the link it came from is sent a skip.
    let Ok(event) = serde_json::from_str::<serde_json::Value>(&json) else { return };
    crate::relay::handlers::federation_log::publish(state, "profile", "", 0, exclude.unwrap_or(""), &event).await;
}

/// The `profile_gossip` frame carrying one signed copy, or None for an unsigned one (never sent).
pub(crate) fn profile_gossip_text(public_key: &str, copy: &PublicCopy, timestamp: u64, signature: &str) -> Option<String> {
    if signature.is_empty() {
        return None;
    }
    serde_json::to_string(&RelayMessage::ProfileGossip {
        public_key: public_key.to_string(),
        name: copy.name.clone(),
        bio: copy.bio.clone(),
        avatar_url: copy.avatar_url.clone(),
        banner_url: copy.banner_url.clone(),
        socials: copy.socials.clone(),
        pronouns: copy.pronouns.clone(),
        location: copy.location.clone(),
        website: copy.website.clone(),
        timestamp,
        signature: signature.to_string(),
    })
    .ok()
}

/// The catch-up when a federation link comes up (BUG-191 plan 3.7): queue onto `peer_id`'s own
/// channel this server's copies for its own accounts (`copies_of_accounts_here`: withdrawals
/// included, newest first, at most 1,000). So a server that was down when someone here made a
/// field friends-only or went unlisted learns it when the link returns; it keeps what is newer
/// than what it holds and refuses the rest. Called by the side that was dialed once the peer is
/// registered (`run_inbound_peer`) and by the dialer on the welcome. Returns how many were queued.
///
/// Once per link: a link is known by when it was registered (`connected_at`), so a peer that sends
/// its welcome again is not sent up to 1,000 copies (about 13 MB) again. A new link always is.
pub async fn send_own_copies(state: &Arc<RelayState>, peer_id: &str) -> usize {
    let Some(link) = state.federation_connections.read().await.get(peer_id).map(|c| c.connected_at) else {
        return 0;
    };
    {
        // federation_rate is this server's per-peer federation bookkeeping; the mark is the one
        // entry under its own key, never counted as a rate.
        let mut marks = state.federation_rate.lock().unwrap();
        let mark = marks.entry(format!("{peer_id}:catch_up")).or_default();
        if mark.as_slice() == [link] {
            return 0;
        }
        *mark = vec![link];
    }
    send_own_copies_again(state, peer_id).await
}

/// The catch-up itself, whatever was sent on this link before: for a peer that asked for it again
/// because it set copies aside while paused, or that was behind more than the outbox keeps
/// (federation_log.rs `serve_resume`). Returns how many were queued.
pub async fn send_own_copies_again(state: &Arc<RelayState>, peer_id: &str) -> usize {
    let copies = match state.db.copies_of_accounts_here() {
        Ok(copies) => copies,
        Err(e) => {
            tracing::error!("Federation: could not read the profile copies to send {} on link up: {e}", key_in_log(peer_id));
            return 0;
        }
    };
    let connections = state.federation_connections.read().await;
    let Some(conn) = connections.get(peer_id) else {
        return 0;
    };
    let mut sent = 0;
    for row in &copies {
        let Some(text) = profile_gossip_text(&row.public_key, &row.copy(), row.timestamp, &row.signature) else {
            continue;
        };
        if conn.tx.send(text).is_err() {
            break;
        }
        sent += 1;
    }
    if sent > 0 {
        tracing::info!("Federation: sent {} this server's profile copies on link up: {}", key_in_log(peer_id), sent);
    }
    sent
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};
    use crate::relay::core::profile_copy::{sign_copy, verify_copy, SIGNATURE_HEX_LEN};

    // A profile copy is Dilithium3-signed by its owner's app (BUG-191). The tests use a fixed
    // seed; the verifier under test is the one production uses (`profile_copy::verify_copy`
    // inside `should_accept_profile_gossip`).
    const SEED: [u8; 32] = [7u8; 32];
    const TS: u64 = 1_700_000_000_000;

    fn key_of(seed: &[u8; 32]) -> String {
        hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(seed)).public_key())
    }

    /// A copy inside every limit.
    fn alice() -> PublicCopy {
        PublicCopy {
            name: "Alice".to_string(),
            bio: "bio".to_string(),
            avatar_url: "https://alice.example/a.png".to_string(),
            banner_url: String::new(),
            socials: r#"{"github":"alice"}"#.to_string(),
            pronouns: "she/her".to_string(),
            location: "Lisbon".to_string(),
            website: "https://alice.example".to_string(),
        }
    }

    #[test]
    fn verify_accepts_valid_signature() {
        let pk = key_of(&SEED);
        let sig = sign_copy(&SEED, &pk, &alice(), TS);
        assert!(verify_copy(&pk, &alice(), TS, &sig));
    }

    #[test]
    fn verify_rejects_forged_signature() {
        let forged = "0".repeat(SIGNATURE_HEX_LEN);
        assert!(!verify_copy(&key_of(&SEED), &alice(), TS, &forged));
    }

    #[test]
    fn verify_rejects_tampered_field() {
        let pk = key_of(&SEED);
        let sig = sign_copy(&SEED, &pk, &alice(), TS);
        // Same signature, different name: fails.
        let mallory = PublicCopy { name: "Mallory".to_string(), ..alice() };
        assert!(!verify_copy(&pk, &mallory, TS, &sig));
        // Same signature, a later time: fails.
        assert!(!verify_copy(&pk, &alice(), TS + 1, &sig));
    }

    #[test]
    fn verify_rejects_wrong_key() {
        // Signed with one key, claimed for another's: fails.
        let other = key_of(&[9u8; 32]);
        let sig = sign_copy(&SEED, &other, &alice(), TS);
        assert!(!verify_copy(&other, &alice(), TS, &sig));
    }

    #[test]
    fn verify_rejects_malformed_inputs() {
        let pk = key_of(&SEED);
        let sig = sign_copy(&SEED, &pk, &alice(), TS);
        assert!(!verify_copy("deadbeef", &alice(), TS, &sig), "a key of the wrong length");
        assert!(!verify_copy("zz", &alice(), TS, &sig), "a key that is not hex");
        assert!(!verify_copy(&pk, &alice(), TS, "deadbeef"), "a signature of the wrong length");
        assert!(!verify_copy(&pk, &alice(), TS, &sig.to_uppercase()), "a signature in upper case (one spelling)");
    }

    #[test]
    fn accept_rejects_empty_signature() {
        // The old trust-by-source model ADMITTED empty signatures, which made an empty string a
        // skeleton key over every cached profile. Closed in the 2026-08-13 federation repair: no
        // signature, no cache write, no exceptions.
        assert!(!should_accept_profile_gossip(&key_of(&SEED), &alice(), TS, ""));
    }

    #[test]
    fn accept_admits_valid_signature() {
        let pk = key_of(&SEED);
        let sig = sign_copy(&SEED, &pk, &alice(), TS);
        assert!(should_accept_profile_gossip(&pk, &alice(), TS, &sig));
        let w = profile_copy::withdrawal();
        let sig = sign_copy(&SEED, &pk, &w, TS + 1);
        assert!(should_accept_profile_gossip(&pk, &w, TS + 1, &sig), "a signed withdrawal");
    }

    #[test]
    fn accept_rejects_invalid_non_empty_signature() {
        let forged = "0".repeat(SIGNATURE_HEX_LEN);
        assert!(!should_accept_profile_gossip(&key_of(&SEED), &alice(), TS, &forged));
    }

    /// A copy its owner really signed is still refused when it breaks the save's limits: a line
    /// break outside the bio would let the signed words split two ways.
    ///
    /// Seen red 2026-10-12 with the `within_limits` call taken out of
    /// `should_accept_profile_gossip`: "a signed copy whose location holds a line break was
    /// accepted" failed.
    #[test]
    fn accept_refuses_a_signed_copy_over_the_limits() {
        let pk = key_of(&SEED);
        let broken = PublicCopy { location: "Lis\nbon".to_string(), ..alice() };
        let sig = sign_copy(&SEED, &pk, &broken, TS);
        assert!(verify_copy(&pk, &broken, TS, &sig), "test setup: the signature itself is good");
        assert!(!should_accept_profile_gossip(&pk, &broken, TS, &sig), "a signed copy whose location holds a line break was accepted");
    }

    /// The frame sent between servers carries the copy's fields and the signature as they are; an
    /// unsigned copy makes no frame at all.
    #[test]
    fn the_gossip_frame_carries_the_copy_and_never_an_unsigned_one() {
        assert!(profile_gossip_text(&key_of(&SEED), &alice(), TS, "").is_none(), "an unsigned copy is never sent");
        let text = profile_gossip_text("k", &alice(), TS, "sig").expect("a frame");
        let wire: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!((wire["type"].as_str(), wire["public_key"].as_str(), wire["location"].as_str()), (Some("profile_gossip"), Some("k"), Some("Lisbon")));
        assert_eq!((wire["socials"].as_str(), wire["timestamp"].as_u64(), wire["signature"].as_str()), (Some(r#"{"github":"alice"}"#), Some(TS), Some("sig")));
    }
}

/// BUG-191 Wave 1B: what a server keeps of the profile copies its links send, what it passes on,
/// and what it sends a new link. A linked server is faked the way a real link registers itself: a
/// `FederatedConnection` whose channel the test reads where the link's write pump would.
#[cfg(test)]
mod receiving_tests {
    use super::*;
    use crate::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};
    use crate::relay::core::profile_copy::{is_withdrawal, sign_copy, withdrawal};
    use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

    /// A person's key: 3,904 lowercase hex characters, a real Dilithium3 public key.
    fn key_of(seed: &[u8; 32]) -> String {
        hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(seed)).public_key())
    }

    /// A minute ago: a real time, since a server keeps nothing dated more than a day ahead of its
    /// clock, with room for the few later times a test adds.
    fn recently() -> u64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64 - 60_000
    }

    fn copy_of(name: &str, location: &str) -> PublicCopy {
        PublicCopy {
            name: name.to_string(),
            bio: "hello".to_string(),
            avatar_url: "https://example.org/a.png".to_string(),
            socials: r#"{"github":"x"}"#.to_string(),
            location: location.to_string(),
            ..PublicCopy::default()
        }
    }

    /// The frame a linked server sends: `copy` of `key`'s profile at `at`, signed by `seed`.
    fn gossip(seed: &[u8; 32], key: &str, copy: &PublicCopy, at: u64) -> RelayMessage {
        RelayMessage::ProfileGossip {
            public_key: key.to_string(),
            name: copy.name.clone(),
            bio: copy.bio.clone(),
            avatar_url: copy.avatar_url.clone(),
            banner_url: copy.banner_url.clone(),
            socials: copy.socials.clone(),
            pronouns: copy.pronouns.clone(),
            location: copy.location.clone(),
            website: copy.website.clone(),
            timestamp: at,
            signature: sign_copy(seed, key, copy, at),
        }
    }

    /// Keep `copy` here as if its owner's app had signed it on this server.
    fn keep(state: &RelayState, seed: &[u8; 32], key: &str, copy: &PublicCopy, at: u64) {
        assert!(state.db.store_signed_profile(key, copy, at, &sign_copy(seed, key, copy, at)).unwrap(), "test setup: the copy is kept");
    }

    /// A fake linked server, registered as a link registers itself; read what it is sent with `sent`.
    async fn link(state: &RelayState, id: &str) -> UnboundedReceiver<String> {
        let (tx, rx) = unbounded_channel();
        state.federation_connections.write().await.insert(
            id.to_string(),
            FederatedConnection { tx, server_id: id.to_string(), server_name: id.to_string(), trust_tier: 2, connected_at: Instant::now() },
        );
        rx
    }

    /// Every frame queued for a fake link since the last look, as it would be applied: a numbered
    /// frame (federation_log.rs, since 2026-10-22) is read as the event it carries, and a skip (a
    /// frame not meant for that link) as nothing.
    fn sent(rx: &mut UnboundedReceiver<String>) -> Vec<serde_json::Value> {
        let mut frames = Vec::new();
        while let Ok(text) = rx.try_recv() {
            let frame: serde_json::Value = serde_json::from_str(&text).expect("a frame is JSON");
            match frame["type"].as_str() {
                Some("fed_event") => frames.push(frame["event"].clone()),
                Some("fed_skip") => {}
                _ => frames.push(frame),
            }
        }
        frames
    }

    fn welcome(id: &str) -> RelayMessage {
        RelayMessage::FederationWelcome { server_id: id.to_string(), name: id.to_string(), channels: Vec::new() }
    }

    /// WHAT A SAVE WOULD REFUSE IS NOT KEPT, even signed by its owner: a location holding a line
    /// break (the signed words could then split two ways), a bio of 281 characters, an avatar link
    /// that is not https. A copy inside the limits, sent the same way, is kept.
    ///
    /// Seen red 2026-10-12 with `within_limits` skipped in `should_accept_profile_gossip`: "a copy
    /// whose location holds a line break was kept".
    #[tokio::test]
    async fn gossip_a_save_would_refuse_is_not_kept() {
        let state = Arc::new(RelayState::new(Storage::open_temp("fed_over_limits")));
        let at = recently();
        let ok = [40u8; 32];
        let ok_key = key_of(&ok);
        handle_peer_message(&state, "peer-b", gossip(&ok, &ok_key, &copy_of("Ann", "Lisbon"), at)).await;
        assert!(state.db.get_signed_profile(&ok_key).unwrap().is_some(), "a copy inside the limits is kept");

        let over = [
            ([41u8; 32], PublicCopy { location: "Lis\nbon".to_string(), ..copy_of("Bo", "") }, "a copy whose location holds a line break was kept"),
            ([42u8; 32], PublicCopy { bio: "a".repeat(281), ..copy_of("Cy", "") }, "a copy with a 281-character bio was kept"),
            ([43u8; 32], PublicCopy { avatar_url: "http://example.org/a.png".to_string(), ..copy_of("Di", "") }, "a copy with an http avatar was kept"),
        ];
        for (seed, copy, said) in &over {
            let key = key_of(seed);
            handle_peer_message(&state, "peer-b", gossip(seed, &key, copy, at)).await;
            assert!(state.db.get_signed_profile(&key).unwrap().is_none(), "{said}");
        }
    }

    /// A WITHDRAWAL FROM A PEER (the person went unlisted on another server) replaces the copy held
    /// here, whole, and nothing else: their own profile on this server and the copies of other
    /// people stay as they were. Their old copy, sent again by another link, does not come back.
    /// A withdrawal for a key never held is kept nowhere.
    ///
    /// Seen red 2026-10-12 with the arm deleting the held copy on a withdrawal instead of keeping
    /// it (the save's old delete-only way of going unlisted): "the withdrawal is held in place of
    /// her copy".
    #[tokio::test]
    async fn a_withdrawal_from_a_peer_replaces_the_copy_held_and_nothing_else() {
        let state = Arc::new(RelayState::new(Storage::open_temp("fed_withdrawal")));
        let (ann, bo, cy) = ([51u8; 32], [52u8; 32], [53u8; 32]);
        let (ann_key, bo_key, cy_key) = (key_of(&ann), key_of(&bo), key_of(&cy));
        state.db.register_name("Ann", &ann_key).unwrap();
        state.db.save_profile_extended("Ann", "hello", "{}", "", "", "she/her", "Lisbon", "", "{}", None).unwrap();
        let t = recently();
        handle_peer_message(&state, "peer-b", gossip(&ann, &ann_key, &copy_of("Ann", "Lisbon"), t)).await;
        handle_peer_message(&state, "peer-b", gossip(&bo, &bo_key, &copy_of("Bo", "Porto"), t)).await;
        assert!(state.db.get_signed_profile(&ann_key).unwrap().is_some(), "test setup: Ann's copy is held");

        handle_peer_message(&state, "peer-b", gossip(&ann, &ann_key, &withdrawal(), t + 1)).await;
        let held = state.db.get_signed_profile(&ann_key).unwrap().expect("the withdrawal is held in place of her copy");
        assert_eq!((held.copy(), held.timestamp), (withdrawal(), t + 1), "the whole row is replaced: nothing of her old copy stays");

        handle_peer_message(&state, "peer-c", gossip(&ann, &ann_key, &copy_of("Ann", "Lisbon"), t)).await;
        assert!(is_withdrawal(&state.db.get_signed_profile(&ann_key).unwrap().unwrap().copy()), "her old copy came back after the withdrawal");

        assert_eq!(state.db.get_signed_profile(&bo_key).unwrap().map(|r| r.copy()), Some(copy_of("Bo", "Porto")), "another person's copy changed");
        let here = state.db.get_profile_extended("Ann").unwrap().expect("her profile on this server stays");
        assert_eq!((here.0.as_str(), here.5.as_str(), here.7.as_str()), ("hello", "Lisbon", "{}"), "a peer's withdrawal changed her profile on this server");

        handle_peer_message(&state, "peer-b", gossip(&cy, &cy_key, &withdrawal(), t + 1)).await;
        assert!(state.db.get_signed_profile(&cy_key).unwrap().is_none(), "a withdrawal for a key never held left a trace");
    }

    /// A NEW LINK IS SENT THIS SERVER'S OWN COPIES (the catch-up, on the welcome): the copies of
    /// keys with an account here, a withdrawal among them, newest first, each one a receiver
    /// keeps; never a copy that only arrived from another server; only on the new link. Once per
    /// link: a second welcome on it sends nothing, and the next link is sent them again.
    ///
    /// Seen red 2026-10-12 with the account filter removed from `copies_of_accounts_here`: "the
    /// copy that only arrived from another server went out on link up".
    #[tokio::test]
    async fn a_new_link_is_sent_this_servers_own_copies() {
        let state = Arc::new(RelayState::new(Storage::open_temp("fed_catch_up")));
        let (ann, bo, far) = ([61u8; 32], [62u8; 32], [63u8; 32]);
        let (ann_key, bo_key, far_key) = (key_of(&ann), key_of(&bo), key_of(&far));
        state.db.register_name("Ann", &ann_key).unwrap();
        state.db.register_name("Bo", &bo_key).unwrap();
        let t = recently();
        keep(&state, &ann, &ann_key, &copy_of("Ann", "Lisbon"), t);
        keep(&state, &bo, &bo_key, &copy_of("Bo", "Porto"), t + 1);
        keep(&state, &bo, &bo_key, &withdrawal(), t + 2);
        keep(&state, &far, &far_key, &copy_of("Far", "Oslo"), t + 3);

        let mut old = link(&state, "peer-b").await;
        let mut new = link(&state, "peer-c").await;
        handle_peer_message(&state, "peer-c", welcome("peer-c")).await;
        let frames = sent(&mut new);
        let keys: Vec<&str> = frames.iter().map(|f| f["public_key"].as_str().unwrap_or("")).collect();
        assert!(!keys.contains(&far_key.as_str()), "the copy that only arrived from another server went out on link up");
        assert_eq!(keys, [bo_key.as_str(), ann_key.as_str()], "this server's accounts, newest first");
        for frame in &frames {
            let Ok(RelayMessage::ProfileGossip { public_key, name, bio, avatar_url, banner_url, socials, pronouns, location, website, timestamp, signature }) =
                serde_json::from_value::<RelayMessage>(frame.clone())
            else {
                panic!("not a profile_gossip frame: {frame}");
            };
            let copy = PublicCopy { name, bio, avatar_url, banner_url, socials, pronouns, location, website };
            assert!(should_accept_profile_gossip(&public_key, &copy, timestamp, &signature), "a receiver would refuse {frame}");
        }
        assert_eq!(frames[0]["name"].as_str(), Some(""), "Bo's withdrawal is sent");
        assert!(sent(&mut old).is_empty(), "the catch-up went to a link that was already up");

        handle_peer_message(&state, "peer-c", welcome("peer-c")).await;
        assert!(sent(&mut new).is_empty(), "a second welcome on the same link was sent everything again");
        let mut again = link(&state, "peer-c").await;
        assert_eq!(send_own_copies(&state, "peer-c").await, 2, "the next link to the same server is sent them again");
        assert_eq!(sent(&mut again).len(), 2);
    }

    /// A COPY KEPT FOR ONE OF OURS GOES ON TO THE OTHER LINKS AND NOT BACK: Ann has an account
    /// here, so her copy from one link goes on to the other, and her withdrawal from that other
    /// goes back the first way. The same copy again is not newer, so it goes nowhere (no loop). A
    /// copy kept for someone with no account here is not passed on.
    ///
    /// Seen red 2026-10-12 with the exclusion removed (`gossip_profile(.., None)` in the arm):
    /// "the copy went back to the server it came from".
    #[tokio::test]
    async fn a_copy_kept_for_one_of_ours_goes_on_to_the_other_links_and_not_back() {
        let state = Arc::new(RelayState::new(Storage::open_temp("fed_pass_on")));
        let (ann, far) = ([71u8; 32], [72u8; 32]);
        let (ann_key, far_key) = (key_of(&ann), key_of(&far));
        state.db.register_name("Ann", &ann_key).unwrap();
        let mut b = link(&state, "peer-b").await;
        let mut c = link(&state, "peer-c").await;
        let t = recently();

        let copy = gossip(&ann, &ann_key, &copy_of("Ann", "Lisbon"), t);
        handle_peer_message(&state, "peer-b", copy.clone()).await;
        let on = sent(&mut c);
        assert_eq!(on.len(), 1, "the copy did not go on to the other link");
        assert_eq!((on[0]["public_key"].as_str(), on[0]["location"].as_str(), on[0]["timestamp"].as_u64()), (Some(ann_key.as_str()), Some("Lisbon"), Some(t)));
        assert!(sent(&mut b).is_empty(), "the copy went back to the server it came from");

        handle_peer_message(&state, "peer-b", copy.clone()).await;
        handle_peer_message(&state, "peer-c", copy).await;
        assert!(sent(&mut b).is_empty() && sent(&mut c).is_empty(), "a copy already held was passed on again");

        handle_peer_message(&state, "peer-c", gossip(&ann, &ann_key, &withdrawal(), t + 1)).await;
        let back = sent(&mut b);
        assert_eq!((back.len(), back.first().and_then(|f| f["name"].as_str())), (1, Some("")), "her withdrawal did not go on to the other link");
        assert!(sent(&mut c).is_empty(), "the withdrawal went back to the server it came from");

        handle_peer_message(&state, "peer-b", gossip(&far, &far_key, &copy_of("Far", "Oslo"), t)).await;
        assert!(state.db.get_signed_profile(&far_key).unwrap().is_some(), "test setup: Far's copy is kept");
        assert!(sent(&mut c).is_empty(), "a copy of someone with no account here was passed on");
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

    /// A KEY IN THE LOG IS ITS FIRST 16 CHARACTERS: a whole server key is 3,904, and every hello
    /// wrote it out (2026-10-12). Seen red with key_in_log returning the key whole: "16 characters
    /// and dots".
    #[test]
    fn a_key_in_the_log_is_its_first_16_characters() {
        let key = "a1".repeat(1952);
        assert_eq!(key_in_log(&key), format!("{}...", &key[..16]), "16 characters and dots");
        assert_eq!(key_in_log("short"), "short");
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
