//! The Federation panel's actions (2026-10-22; docs/design/in-app-ops.md, "Federation peers",
//! and docs/design/federation-activation.md Phase 1).
//!
//! One implementation for every way in: the registry's `federation.*` actions run here, and the
//! older `POST /api/admin/federation` (api.rs) hands its actions to the same functions, so a
//! peer added by either is the same row with the same checks. The chat commands (`/server-add`
//! and the rest, relay.rs) predate both and still carry their own copy.
//!
//! WHAT THE PANEL READS ([`view`]). Every peer with its trust, whether it is paused, whether a
//! link to it is open now (the live connections, read only) and since when, its status and when
//! it was last heard from (`last_seen`, which every message from the peer moves); every channel
//! with whether it is federated and whether it is the local-only room; and this server's own
//! federation key, for another server's admin to add by key.
//!
//! PAUSE (storage/federation_pause.rs) keeps the peer and its trust while federation stops,
//! through the registry read every federation path already makes; nothing in
//! handlers/federation.rs changed for it.

use std::sync::Arc;

use serde_json::{json, Value};

use super::{Caller, Inputs, Ran, Refusal};
use crate::relay::handlers::federation as fed;
use crate::relay::relay::{RelayMessage, RelayState};

/// What the Federation panel draws: peers, channels, this server's key.
pub async fn view(state: &RelayState) -> Value {
    let rows = state.db.list_federated_servers_all().unwrap_or_default();
    let links: Vec<(String, u64)> = state
        .federation_connections
        .read()
        .await
        .values()
        .map(|c| (c.server_id.clone(), c.connected_at.elapsed().as_secs()))
        .collect();
    let peers: Vec<Value> = rows
        .iter()
        .map(|(s, paused)| {
            let link = links.iter().find(|(id, _)| id == &s.server_id || id == &s.url || Some(id) == s.public_key.as_ref());
            json!({
                "server_id": s.server_id,
                "name": s.name,
                "url": s.url,
                "by_key": s.url == "outbound-only",
                "has_key": s.public_key.is_some(),
                "trust_tier": s.trust_tier,
                "accord_compliant": s.accord_compliant,
                "status": s.status,
                "last_seen": s.last_seen,
                "added_at": s.added_at,
                "paused": paused,
                "federates": !paused && s.trust_tier >= 2,
                "linked": link.is_some(),
                "linked_secs": link.map(|(_, secs)| *secs),
                // The link's own state (Federation Phases 2 and 3, handlers/federation_log.rs
                // `peer_view`, shape in docs/design/federation-activation.md): its flow (ok,
                // slowed, or held back automatically for sending too fast; not the admin pause
                // above), frames set aside and missed, the log positions, the peer's bans.
                "link": crate::relay::handlers::federation_log::peer_view(state, s.public_key.as_deref()),
            })
        })
        .collect();
    let federated = state.db.get_federated_channels().unwrap_or_default();
    let guaranteed = state.db.get_server_settings().map(|s| s.local_channel_enabled).unwrap_or(true);
    let channels: Vec<Value> = state
        .db
        .list_channels()
        .unwrap_or_default()
        .into_iter()
        .map(|(id, name, _desc, _ro)| {
            let local_only = state.db.is_channel_local_only(&id).unwrap_or(false);
            json!({
                "id": id,
                "name": name,
                "federated": federated.contains(&id),
                "local_only": local_only,
                "can_federate": !(local_only && guaranteed),
            })
        })
        .collect();
    json!({
        "peers": peers,
        "channels": channels,
        "own_key": state.db.get_or_create_server_keypair().map(|(pk, _)| pk).unwrap_or_default(),
        "local_room_guaranteed": guaranteed,
    })
}

/// A peer's name for a message: its name, or its id when it has none.
fn peer_name(state: &RelayState, server_id: &str) -> Option<(String, bool)> {
    state
        .db
        .list_federated_servers_all()
        .ok()?
        .into_iter()
        .find(|(s, _)| s.server_id == server_id)
        .map(|(s, paused)| (if s.name.is_empty() { s.server_id.clone() } else { s.name.clone() }, paused))
}

fn server_id(inputs: &Inputs) -> Result<String, Refusal> {
    inputs.text("server_id").ok_or_else(|| Refusal::bad("Which peer? Give its id from the list."))
}

async fn done(state: &RelayState, message: String, log: String) -> Result<Ran, Refusal> {
    Ok(Ran::said(message).logged_as(log).with_data(view(state).await))
}

/// `federation.list`
pub async fn list(state: Arc<RelayState>, _caller: Caller, _inputs: Inputs) -> Result<Ran, Refusal> {
    let data = view(&state).await;
    let n = data["peers"].as_array().map_or(0, |p| p.len());
    let linked = data["peers"].as_array().map_or(0, |p| p.iter().filter(|x| x["linked"] == true).count());
    let message = if n == 0 {
        "No peers yet. This server works on its own until you add one.".to_string()
    } else {
        format!("{n} peer{}, {linked} linked now.", if n == 1 { "" } else { "s" })
    };
    Ok(Ran::said(message).with_data(data))
}

/// `federation.add`
pub async fn add(state: Arc<RelayState>, _caller: Caller, inputs: Inputs) -> Result<Ran, Refusal> {
    let url = inputs.text("url").unwrap_or_default();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(Refusal::bad("URL must start with http:// or https://"));
    }
    // The URL is the server_id, matching /server-add exactly, so a peer added here and a peer
    // added by command are the same row.
    let id = url.trim_end_matches('/').to_string();
    let name = inputs.text("name").unwrap_or_else(|| id.clone());
    match state.db.add_federated_server(&id, &name, &url) {
        Ok(true) => {
            spawn_peer_discovery(&state, &id, &url);
            done(
                &state,
                format!("Added {name}. It stays at trust tier 0 until you raise it, and nothing federates below tier 2."),
                format!("Added peer {id}."),
            )
            .await
        }
        Ok(false) => Err(Refusal::with(axum::http::StatusCode::CONFLICT, "That server is already in the registry.")),
        Err(e) => Err(Refusal::failed(format!("Database error: {e}"))),
    }
}

/// `federation.add_key`: the NAT-friendly half. A home node behind a router has no URL we can
/// fetch, but it can dial out; pinning its key lets its inbound hello verify. "outbound-only" is
/// a deliberate non-http marker so the outbound dialer skips the row.
pub async fn add_key(state: Arc<RelayState>, _caller: Caller, inputs: Inputs) -> Result<Ran, Refusal> {
    let key = inputs.text("key").unwrap_or_default().to_lowercase();
    if !fed::is_server_key(&key) {
        return Err(Refusal::bad(format!(
            "A server key is {} hex characters (copy it from the other node's Host Node page).",
            fed::SERVER_KEY_HEX_LEN
        )));
    }
    let name = inputs.text("name").unwrap_or_else(|| format!("peer-{}", &key[..8]));
    match state.db.add_federated_server(&key, &name, "outbound-only") {
        Ok(true) => {
            // The key IS the identity here, handed over out of band by the operators themselves.
            let _ = state.db.update_federated_server_info(&key, &name, Some(&key), false);
            done(
                &state,
                format!("Added {name} by key. Raise it to trust tier 2, then have the other node add THIS server by URL. No port forwarding needed on their side."),
                format!("Added peer \"{name}\" by key {}.", fed::key_in_log(&key)),
            )
            .await
        }
        Ok(false) => Err(Refusal::with(axum::http::StatusCode::CONFLICT, "That key is already in the registry.")),
        Err(e) => Err(Refusal::failed(format!("Database error: {e}"))),
    }
}

/// `federation.trust`
pub async fn trust(state: Arc<RelayState>, _caller: Caller, inputs: Inputs) -> Result<Ran, Refusal> {
    let id = server_id(&inputs)?;
    let tier: i32 = inputs.text("tier").and_then(|t| t.parse().ok()).unwrap_or(-1);
    if !(0..=3).contains(&tier) {
        return Err(Refusal::bad("Trust tier must be 0, 1, 2 or 3."));
    }
    match state.db.set_server_trust_tier(&id, tier) {
        Ok(true) => {
            let label = match tier {
                3 => "verified and Accord-compliant",
                2 => "verified",
                1 => "Accord-compliant but unverified",
                _ => "untrusted",
            };
            // Say the consequence, not just the number: tier 2 is the line where federation
            // actually begins (federation.rs skips anything below it, and hellos are refused).
            let effect = if tier >= 2 { "Federation with this peer is now permitted." } else { "Federation with this peer is now refused (tier 2 is the minimum)." };
            let paused = peer_name(&state, &id).is_some_and(|(_, p)| p);
            let note = if paused && tier >= 2 { " It is paused, so nothing links until you resume it." } else { "" };
            done(&state, format!("Set trust tier {tier} ({label}). {effect}{note}"), format!("Set {id} to trust tier {tier}.")).await
        }
        Ok(false) => Err(Refusal::not_found("No such server in the registry.")),
        Err(e) => Err(Refusal::failed(format!("Database error: {e}"))),
    }
}

/// `federation.pause`
pub async fn pause(state: Arc<RelayState>, _caller: Caller, inputs: Inputs) -> Result<Ran, Refusal> {
    let id = server_id(&inputs)?;
    let Some((name, already)) = peer_name(&state, &id) else {
        return Err(Refusal::not_found("No such server in the registry."));
    };
    if already {
        return Err(Refusal::bad(format!("{name} is already paused.")));
    }
    match state.db.pause_federated_server(&id) {
        Ok(true) => {
            fed::broadcast_federation_status(&state).await;
            done(
                &state,
                format!("Paused {name}. An open link to it closes within {} seconds, and none opens again until you resume it. Its trust is kept.", fed::REGISTRY_CHECK_SECS),
                format!("Paused peer {id}."),
            )
            .await
        }
        Ok(false) => Err(Refusal::not_found("No such server in the registry.")),
        Err(e) => Err(Refusal::failed(format!("Database error: {e}"))),
    }
}

/// `federation.resume`
pub async fn resume(state: Arc<RelayState>, _caller: Caller, inputs: Inputs) -> Result<Ran, Refusal> {
    let id = server_id(&inputs)?;
    let Some((name, _)) = peer_name(&state, &id) else {
        return Err(Refusal::not_found("No such server in the registry."));
    };
    match state.db.resume_federated_server(&id) {
        Ok(true) => {
            fed::broadcast_federation_status(&state).await;
            done(
                &state,
                format!("Resumed {name}. It links again the next time either server connects; Connect to trusted peers does it now."),
                format!("Resumed peer {id}."),
            )
            .await
        }
        Ok(false) => Err(Refusal::bad(format!("{name} was not paused."))),
        Err(e) => Err(Refusal::failed(format!("Database error: {e}"))),
    }
}

/// `federation.recheck`: re-read a peer's key from its own server-info and pin it, for a peer
/// whose key changed (every server's did on 2026-09-06).
pub async fn recheck(state: Arc<RelayState>, _caller: Caller, inputs: Inputs) -> Result<Ran, Refusal> {
    let id = server_id(&inputs)?;
    match peer_name(&state, &id) {
        None => return Err(Refusal::not_found("No such server in the registry.")),
        Some((name, true)) => return Err(Refusal::bad(format!("{name} is paused: resume it first, then re-check its key."))),
        Some(_) => {}
    }
    let found = fed::recheck_peer_key(&state, &id).await;
    let words = fed::recheck_words(&found);
    match found {
        Ok(_) => done(&state, words.clone(), format!("Re-checked the key of {id}.")).await,
        Err(_) => Err(Refusal::with(axum::http::StatusCode::BAD_GATEWAY, words)),
    }
}

/// `federation.remove`
pub async fn remove(state: Arc<RelayState>, _caller: Caller, inputs: Inputs) -> Result<Ran, Refusal> {
    let id = server_id(&inputs)?;
    match state.db.remove_federated_server(&id) {
        Ok(true) => done(&state, "Removed. This server no longer federates with that peer.".into(), format!("Removed peer {id}.")).await,
        Ok(false) => Err(Refusal::not_found("No such server in the registry.")),
        Err(e) => Err(Refusal::failed(format!("Database error: {e}"))),
    }
}

/// `federation.connect`
pub async fn connect(state: Arc<RelayState>, _caller: Caller, _inputs: Inputs) -> Result<Ran, Refusal> {
    let n = fed::start_federation_connections(&state).await;
    fed::broadcast_federation_status(&state).await;
    done(
        &state,
        format!("Opening links to {n} trusted peer{}. Refresh in a few seconds to see them linked.", if n == 1 { "" } else { "s" }),
        format!("Started links to {n} trusted peers."),
    )
    .await
}

/// `federation.channel`: set (not toggle) whether a channel federates.
pub async fn channel(state: Arc<RelayState>, _caller: Caller, inputs: Inputs) -> Result<Ran, Refusal> {
    let id = inputs.text("channel").map(|c| c.trim_start_matches('#').to_lowercase()).unwrap_or_default();
    let on = inputs.flag("federated").unwrap_or(false);
    match state.db.set_channel_federated(&id, on) {
        Ok(true) => {
            crate::relay::handlers::broadcast::broadcast_channel_list(&state);
            let words = if on { "federated" } else { "no longer federated" };
            let _ = state.broadcast_tx.send(RelayMessage::System { message: format!("Channel #{id} is now {words}.") });
            done(
                &state,
                format!("#{id} is now {words}.{}", if on { " Its new messages travel to every linked peer that federates it too." } else { "" }),
                format!("#{id} {words}."),
            )
            .await
        }
        Ok(false) if on && state.db.is_channel_local_only(&id).unwrap_or(false) => Err(Refusal::bad(format!(
            "#{id} is this server's guaranteed local-only room, so it never federates while that guarantee is on (Server Settings > ADMIN > Federation)."
        ))),
        Ok(false) => Err(Refusal::not_found(format!("There is no channel #{id}."))),
        Err(e) => Err(Refusal::failed(format!("Database error: {e}"))),
    }
}

/// Fire-and-forget peer discovery after an add: fetch the peer's `/api/server-info` to fill in its
/// real name, key and Accord flag. Never blocks the response, and a failure just marks the row
/// unreachable. Mirrors what `/server-add` does in relay.rs so every path behaves identically.
pub(crate) fn spawn_peer_discovery(state: &Arc<RelayState>, server_id: &str, url: &str) {
    let info_url = format!("{}/api/server-info", url.trim_end_matches('/'));
    let sid = server_id.to_string();
    let state = state.clone();
    tokio::spawn(async move {
        match state.http_client.get(&info_url).timeout(std::time::Duration::from_secs(10)).send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(info) = resp.json::<Value>().await {
                    let name = info["name"].as_str().unwrap_or("Unknown");
                    let pk = info["public_key"].as_str();
                    let accord = info["accord_compliant"].as_bool().unwrap_or(false);
                    let _ = state.db.update_federated_server_info(&sid, name, pk, accord);
                    tracing::info!("Discovered federated server: {} ({})", name, sid);
                }
            }
            Ok(resp) => {
                let _ = state.db.update_federated_server_status(&sid, "unreachable");
                tracing::warn!("Server-info fetch failed for {}: HTTP {}", sid, resp.status());
            }
            Err(e) => {
                let _ = state.db.update_federated_server_status(&sid, "unreachable");
                tracing::warn!("Server-info fetch failed for {}: {}", sid, e.without_url());
            }
        }
    });
}
