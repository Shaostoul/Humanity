//! Broadcast and state-update helper functions extracted from relay.rs.
//! These functions push state changes to connected WebSocket clients.

use std::sync::Arc;

use ed25519_dalek::{Signature, VerifyingKey};

use crate::relay::{
    CategoryInfo, ChannelInfo, DmConversationData, PeerInfo, RelayMessage, RelayState,
    TaskData, UserInfo, VoiceChannelData, VoiceRoomParticipant,
};

/// Broadcast updated channel list (with categories) to all clients.
pub fn broadcast_channel_list(state: &Arc<RelayState>) {
    let infos = build_channel_list(&state.db);
    let categories: Vec<CategoryInfo> = state.db.list_categories().unwrap_or_default().into_iter()
        .map(|(id, name, pos)| CategoryInfo { id, name, position: pos }).collect();
    let _ = state.broadcast_tx.send(RelayMessage::ChannelList { channels: infos, categories: Some(categories) });
}

/// Build the task list with comment counts (helper).
pub fn build_task_list(db: &crate::storage::Storage) -> Vec<TaskData> {
    let tasks = db.list_tasks().unwrap_or_default();
    let counts = db.get_task_comment_counts().unwrap_or_default();
    tasks.into_iter().map(|t| TaskData {
        id: t.id, title: t.title, description: t.description, status: t.status,
        priority: t.priority, assignee: t.assignee, created_by: t.created_by,
        created_at: t.created_at, updated_at: t.updated_at, position: t.position,
        labels: t.labels, comment_count: *counts.get(&t.id).unwrap_or(&0),
    }).collect()
}

/// Build ChannelInfo list with category data from the database.
pub fn build_channel_list(db: &crate::storage::Storage) -> Vec<ChannelInfo> {
    let categories = db.list_categories().unwrap_or_default();
    let cat_map: std::collections::HashMap<i64, String> = categories.into_iter().map(|(id, name, _)| (id, name)).collect();
    let channels = db.list_channels_with_categories().unwrap_or_default();
    channels.into_iter().map(|(id, name, desc, ro, cat_id)| {
        let cat_name = cat_id.and_then(|cid| cat_map.get(&cid).cloned());
        let federated = db.is_channel_federated(&id).unwrap_or(false);
        ChannelInfo { id, name, description: desc, read_only: ro, category_id: cat_id, category_name: cat_name, federated }
    }).collect()
}

/// H-1: Verify an Ed25519 signature. Returns true if valid.
/// Format: sign(content + '\n' + timestamp_string)
pub fn verify_ed25519_signature(public_key_hex: &str, content: &str, timestamp: u64, sig_hex: &str) -> bool {
    let Ok(pk_bytes) = hex::decode(public_key_hex) else { return false };
    if pk_bytes.len() != 32 { return false; }
    let pk_array: [u8; 32] = match pk_bytes.try_into() {
        Ok(a) => a,
        Err(_) => return false,
    };
    let Ok(verifying_key) = VerifyingKey::from_bytes(&pk_array) else { return false };

    let Ok(sig_bytes) = hex::decode(sig_hex) else { return false };
    if sig_bytes.len() != 64 { return false; }
    let sig_array: [u8; 64] = match sig_bytes.try_into() {
        Ok(a) => a,
        Err(_) => return false,
    };
    let signature = Signature::from_bytes(&sig_array);

    let message = format!("{}\n{}", content, timestamp);
    use ed25519_dalek::Verifier;
    verifying_key.verify(message.as_bytes(), &signature).is_ok()
}

/// Broadcast an updated peer list to all connected clients.
/// WHY: After role changes (verify, mod, etc.) clients need fresh data for badges.
pub async fn broadcast_peer_list(state: &Arc<RelayState>) {
    let peers_raw: Vec<crate::relay::Peer> = state
        .peers
        .read()
        .await
        .values()
        .cloned()
        .collect();
    let statuses = state.user_statuses.read().await;
    let peers: Vec<PeerInfo> = peers_raw.into_iter()
        .map(|p| {
            let role = state.db.get_role(&p.public_key_hex).unwrap_or_default();
            let name_lower = p.display_name.as_ref().map(|n| n.to_lowercase()).unwrap_or_default();
            let (user_status, user_status_text) = statuses.get(&name_lower).cloned().unwrap_or(("online".to_string(), String::new()));
            let ecdh_pub = p.ecdh_public.clone().or_else(|| state.db.get_ecdh_public(&p.public_key_hex).ok().flatten());
            PeerInfo {
                public_key: p.public_key_hex.clone(),
                display_name: p.display_name.clone(),
                role,
                upload_token: None,
                status: user_status,
                status_text: user_status_text,
                ecdh_public: ecdh_pub,
            }
        })
        .collect();
    drop(statuses);
    let _ = state.broadcast_tx.send(RelayMessage::PeerList { peers, server_version: None });
}

/// Broadcast the full user list (online + offline) to all connected clients.
pub async fn broadcast_full_user_list(state: &Arc<RelayState>) {
    if let Ok(all_users) = state.db.list_all_users_with_keys() {
        let online_names: std::collections::HashSet<String> = state
            .peers
            .read()
            .await
            .values()
            .filter_map(|p| p.display_name.clone())
            .map(|n| n.to_lowercase())
            .collect();

        let statuses = state.user_statuses.read().await;
        let users: Vec<UserInfo> = all_users
            .into_iter()
            .map(|(name, first_key, role, key_count)| {
                // Bot accounts (bot_ prefix keys) are always shown as online.
                let online = first_key.starts_with("bot_") || online_names.contains(&name.to_lowercase());
                let (user_status, user_status_text) = statuses.get(&name.to_lowercase()).cloned().unwrap_or(("online".to_string(), String::new()));
                let ecdh_pub = state.db.get_ecdh_public(&first_key).ok().flatten();
                UserInfo { name, public_key: first_key, role, online, key_count, status: user_status, status_text: user_status_text, ecdh_public: ecdh_pub }
            })
            .collect();
        drop(statuses);

        let _ = state.broadcast_tx.send(RelayMessage::FullUserList { users });
    }
}

/// Handle moderation commands. Returns a status message for the caller.
pub async fn handle_mod_command(
    state: &Arc<RelayState>,
    cmd: &str,
    caller_role: &str,
    target_name: &str,
    _caller_key: &str,
) -> String {
    // Resolve target name → public key(s).
    let target_keys = match state.db.keys_for_name(target_name) {
        Ok(keys) if !keys.is_empty() => keys,
        _ => return format!("User '{}' not found.", target_name),
    };

    let is_admin = caller_role == "admin";
    let is_mod = caller_role == "mod" || is_admin;

    match cmd {
        "/kick" => {
            if !is_mod { return "You need moderator permissions.".to_string(); }
            // Mark keys as kicked so their WebSocket loops will close.
            {
                let mut kicked = state.kicked_keys.write().await;
                for key in &target_keys {
                    kicked.insert(key.clone());
                }
            }
            // Disconnect all sessions for this name by broadcasting a kick.
            let kick_msg = RelayMessage::System {
                message: format!("{} was kicked.", target_name),
            };
            let _ = state.broadcast_tx.send(kick_msg);
            // Remove from connected peers.
            let mut peers = state.peers.write().await;
            for key in &target_keys {
                peers.remove(key);
            }
            format!("Kicked {}.", target_name)
        }
        "/ban" => {
            if !is_admin { return "Only admins can ban users.".to_string(); }
            for key in &target_keys {
                if let Err(e) = state.db.set_banned(key, true) {
                    tracing::error!("Failed to ban: {e}");
                }
            }
            // Mark keys as kicked so their WebSocket loops will close.
            {
                let mut kicked = state.kicked_keys.write().await;
                for key in &target_keys {
                    kicked.insert(key.clone());
                }
            }
            // Also kick them.
            let mut peers = state.peers.write().await;
            for key in &target_keys {
                peers.remove(key);
            }
            let ban_msg = RelayMessage::System {
                message: format!("{} was banned.", target_name),
            };
            let _ = state.broadcast_tx.send(ban_msg);
            format!("Banned {}.", target_name)
        }
        "/unban" => {
            if !is_admin { return "Only admins can unban users.".to_string(); }
            for key in &target_keys {
                if let Err(e) = state.db.set_banned(key, false) {
                    tracing::error!("Failed to unban: {e}");
                }
            }
            format!("Unbanned {}.", target_name)
        }
        "/mod" => {
            if !is_admin { return "Only admins can assign moderators.".to_string(); }
            for key in &target_keys {
                if let Err(e) = state.db.set_role(key, "mod") {
                    tracing::error!("Failed to set mod: {e}");
                }
            }
            format!("{} is now a moderator.", target_name)
        }
        "/unmod" => {
            if !is_admin { return "Only admins can remove moderators.".to_string(); }
            for key in &target_keys {
                if let Err(e) = state.db.set_role(key, "user") {
                    tracing::error!("Failed to unmod: {e}");
                }
            }
            format!("{} is no longer a moderator.", target_name)
        }
        "/mute" => {
            if !is_mod { return "You need moderator permissions.".to_string(); }
            for key in &target_keys {
                if let Err(e) = state.db.set_role(key, "muted") {
                    tracing::error!("Failed to mute: {e}");
                }
            }
            format!("{} has been muted.", target_name)
        }
        "/unmute" => {
            if !is_mod { return "You need moderator permissions.".to_string(); }
            for key in &target_keys {
                if let Err(e) = state.db.set_role(key, "user") {
                    tracing::error!("Failed to unmute: {e}");
                }
            }
            format!("{} has been unmuted.", target_name)
        }
        _ => "Unknown moderation command.".to_string(),
    }
}

/// Build the voice channel list message (persistent channels + active participants).
pub async fn build_voice_channel_list_msg(state: &Arc<RelayState>) -> RelayMessage {
    let db_channels = state.db.list_voice_channels().unwrap_or_default();
    let rooms = state.voice_rooms.read().await;
    let channels: Vec<VoiceChannelData> = db_channels.into_iter().map(|vc| {
        let rid = vc.id.to_string();
        let participants = rooms.get(&rid).map(|r| {
            r.participants.iter().map(|(k, n)| VoiceRoomParticipant {
                public_key: k.clone(), display_name: n.clone(), muted: false,
            }).collect()
        }).unwrap_or_default();
        VoiceChannelData { id: vc.id, name: vc.name, participants }
    }).collect();
    drop(rooms);
    RelayMessage::VoiceChannelList { channels }
}

/// Broadcast persistent voice channel list to all connected clients.
pub async fn broadcast_voice_channel_list(state: &Arc<RelayState>) {
    let msg = build_voice_channel_list_msg(state).await;
    let _ = state.broadcast_tx.send(msg);
}

/// Broadcast voice room state to all connected clients (legacy, kept for compatibility).
pub async fn broadcast_voice_rooms(state: &Arc<RelayState>) {
    // Now we broadcast the persistent voice channel list instead.
    broadcast_voice_channel_list(state).await;
}

/// Remove a user from any voice room they're in.
pub async fn leave_voice_room(state: &Arc<RelayState>, key: &str) {
    let mut rooms = state.voice_rooms.write().await;
    let mut empty_rooms = Vec::new();
    for (rid, room) in rooms.iter_mut() {
        room.participants.retain(|(k, _)| k != key);
        if room.participants.is_empty() {
            empty_rooms.push(rid.clone());
        }
    }
    // Remove empty in-memory rooms but keep channels in DB.
    for rid in empty_rooms {
        rooms.remove(&rid);
    }
    drop(rooms);
    broadcast_voice_channel_list(state).await;
}

/// Send an updated DM conversation list to a specific user via broadcast (filtered by send loop).
pub fn send_dm_list_update(state: &Arc<RelayState>, user_key: &str) {
    match state.db.get_dm_conversations(user_key) {
        Ok(convos) => {
            let conversations: Vec<DmConversationData> = convos.into_iter().map(|c| DmConversationData {
                partner_key: c.partner_key,
                partner_name: c.partner_name,
                last_message: c.last_message,
                last_timestamp: c.last_timestamp,
                unread_count: c.unread_count,
            }).collect();
            let _ = state.broadcast_tx.send(RelayMessage::DmList {
                target: Some(user_key.to_string()),
                conversations,
            });
        }
        Err(e) => {
            tracing::error!("Failed to get DM conversations for {}: {e}", user_key);
        }
    }
}
