//! Message handler functions extracted from handle_connection's match arms.
//! Each function handles one logical group of WebSocket message types.
//! Pure refactor — no behavior changes.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use crate::relay::*;
use crate::storage::Storage;
use crate::handlers::broadcast::*;
use crate::handlers::utils::*;

// ── Sync handlers (raw JSON, not RelayMessage enum) ──

pub async fn handle_sync_save(
    state: &Arc<RelayState>,
    my_key: &str,
    raw: &serde_json::Value,
) {
    if let Some(data) = raw.get("data").and_then(|d| d.as_str()) {
        if data.len() > 512 * 1024 {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: "Sync data too large (max 512KB).".to_string(),
            };
            let _ = state.broadcast_tx.send(private);
        } else if serde_json::from_str::<serde_json::Value>(data).is_ok() {
            if let Err(e) = state.db.save_user_data(my_key, data) {
                tracing::error!("Failed to save user data: {e}");
            }
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: "sync_ack".to_string(),
            };
            let _ = state.broadcast_tx.send(private);
        }
    }
}

pub async fn handle_sync_load(
    state: &Arc<RelayState>,
    my_key: &str,
) {
    match state.db.load_user_data(my_key) {
        Ok(Some((data, updated_at))) => {
            let resp = serde_json::json!({
                "type": "sync_data",
                "data": data,
                "updated_at": updated_at
            });
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: format!("__sync_data__:{}", resp.to_string()),
            };
            let _ = state.broadcast_tx.send(private);
        }
        Ok(None) => {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: "__sync_data__:null".to_string(),
            };
            let _ = state.broadcast_tx.send(private);
        }
        Err(e) => {
            tracing::error!("Failed to load user data: {e}");
        }
    }
}

// ── Key rotation handler ──

pub async fn handle_key_rotation(
    state: &Arc<RelayState>,
    my_key: &str,
    raw: &serde_json::Value,
) {
    if let (Some(new_key), Some(sig_by_old), Some(sig_by_new), Some(ts)) = (
        raw.get("new_key").and_then(|v| v.as_str()),
        raw.get("sig_by_old").and_then(|v| v.as_str()),
        raw.get("sig_by_new").and_then(|v| v.as_str()),
        raw.get("timestamp").and_then(|v| v.as_u64()),
    ) {
        let old_key = my_key;
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default().as_millis() as u64;
        let fresh = now_ms.saturating_sub(ts) < 5 * 60 * 1000;
        let ok_old = verify_ed25519_signature(old_key, new_key, ts, sig_by_old);
        let ok_new = verify_ed25519_signature(new_key, old_key, ts, sig_by_new);
        if fresh && ok_old && ok_new {
            match state.db.record_key_rotation(old_key, new_key, sig_by_old, sig_by_new) {
                Ok(()) => {
                    let notif = serde_json::json!({
                        "type": "key_rotated",
                        "old_key": old_key,
                        "new_key": new_key,
                        "timestamp": ts
                    }).to_string();
                    let _ = state.broadcast_tx.send(
                        RelayMessage::System { message: notif }
                    );
                    tracing::info!("Key rotation: {old_key:.16}… → {new_key:.16}…");
                }
                Err(e) => tracing::error!("Key rotation DB error: {e}"),
            }
        } else {
            tracing::warn!("Key rotation rejected for {old_key:.16}… fresh={fresh} ok_old={ok_old} ok_new={ok_new}");
        }
    }
}

// ── Skill handlers ──

pub async fn handle_skill_update(
    state: &Arc<RelayState>,
    my_key: &str,
    raw: &serde_json::Value,
) {
    if let (Some(skill_id), Some(reality_xp), Some(fantasy_xp), Some(level)) = (
        raw.get("skill_id").and_then(|v| v.as_str()),
        raw.get("reality_xp").and_then(|v| v.as_f64()),
        raw.get("fantasy_xp").and_then(|v| v.as_f64()),
        raw.get("level").and_then(|v| v.as_i64()),
    ) {
        let _ = state.db.upsert_skill(my_key, skill_id, reality_xp, fantasy_xp, level as i32);
    }
}

pub async fn handle_skill_verify_request(
    state: &Arc<RelayState>,
    my_key: &str,
    raw: &serde_json::Value,
) {
    if let (Some(skill_id), Some(to_name)) = (
        raw.get("skill_id").and_then(|v| v.as_str()),
        raw.get("to_name").and_then(|v| v.as_str()),
    ) {
        let level = raw.get("level").and_then(|v| v.as_i64()).unwrap_or(0);
        let from_name = {
            let peers = state.peers.read().await;
            peers.get(my_key).and_then(|p| p.display_name.clone()).unwrap_or_else(|| "Someone".to_string())
        };
        // Find target key by name
        if let Ok(Some(true)) = state.db.check_name(to_name, "") {
            // Name exists but belongs to someone else — good, find them
        }
        let target_key = {
            let peers = state.peers.read().await;
            peers.iter().find(|(_, p)| p.display_name.as_deref() == Some(to_name)).map(|(k, _)| k.clone())
        };
        if let Some(tk) = target_key {
            let msg = format!("__skill_verify_req__:{{\"from_key\":\"{}\",\"from_name\":\"{}\",\"skill_id\":\"{}\",\"level\":{}}}", my_key, from_name, skill_id, level);
            let private = RelayMessage::Private {
                to: tk,
                message: msg,
            };
            let _ = state.broadcast_tx.send(private);
        }
    }
}

pub async fn handle_skill_verify_response(
    state: &Arc<RelayState>,
    my_key: &str,
    raw: &serde_json::Value,
) {
    if let (Some(skill_id), Some(to_key), Some(approved)) = (
        raw.get("skill_id").and_then(|v| v.as_str()),
        raw.get("to_key").and_then(|v| v.as_str()),
        raw.get("approved").and_then(|v| v.as_bool()),
    ) {
        if approved {
            let note = raw.get("note").and_then(|v| v.as_str()).unwrap_or("Verified");
            let _ = state.db.store_skill_verification(skill_id, my_key, to_key, note);
            let from_name = {
                let peers = state.peers.read().await;
                peers.get(my_key).and_then(|p| p.display_name.clone()).unwrap_or_else(|| "Someone".to_string())
            };
            let msg = format!("__skill_verify_resp__:{{\"from_key\":\"{}\",\"from_name\":\"{}\",\"skill_id\":\"{}\",\"approved\":true,\"note\":\"{}\"}}", my_key, from_name, skill_id, note);
            let private = RelayMessage::Private {
                to: to_key.to_string(),
                message: msg,
            };
            let _ = state.broadcast_tx.send(private);
        }
    }
}

pub async fn handle_skill_endorsements_request(
    state: &Arc<RelayState>,
    my_key: &str,
    raw: &serde_json::Value,
) {
    let target_key = raw.get("user_key")
        .and_then(|v| v.as_str())
        .unwrap_or(my_key)
        .to_string();
    if let Ok(counts) = state.db.get_skill_endorsement_counts(&target_key) {
        let entries: Vec<serde_json::Value> = counts.into_iter().map(|(skill_id, count, endorser)| {
            serde_json::json!({ "skill_id": skill_id, "count": count, "last_endorser": endorser })
        }).collect();
        let payload = serde_json::json!({ "type": "skill_endorsements", "user_key": target_key, "endorsements": entries }).to_string();
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: format!("__skill_endorsements__:{}", payload),
        };
        let _ = state.broadcast_tx.send(private);
    }
}

// ── Raw JSON task handlers (from the raw JSON type match, not RelayMessage) ──

pub async fn handle_raw_task_update(
    state: &Arc<RelayState>,
    my_key: &str,
    raw: &serde_json::Value,
) {
    if let Some(task_id) = raw.get("task_id").and_then(|v| v.as_i64()) {
        let valid_statuses  = ["backlog", "in_progress", "testing", "done"];
        let valid_priorities = ["low", "medium", "high", "critical"];
        let new_status   = raw.get("status").and_then(|v| v.as_str()).filter(|s| valid_statuses.contains(s));
        let new_priority = raw.get("priority").and_then(|v| v.as_str()).filter(|p| valid_priorities.contains(p));
        let new_title    = raw.get("title").and_then(|v| v.as_str());
        let assignee_present = raw.get("assignee").is_some();
        let new_assignee_str = raw.get("assignee").and_then(|v| v.as_str());
        if let Ok(Some(task)) = state.db.get_task(task_id) {
            if let Some(ns) = new_status {
                if ns != task.status.as_str() {
                    let _ = state.db.move_task(task_id, ns);
                }
            }
            if new_priority.is_some() || new_title.is_some() || assignee_present {
                let priority = new_priority.unwrap_or(&task.priority);
                let title    = new_title.unwrap_or(&task.title);
                let assignee = if assignee_present {
                    new_assignee_str.filter(|s| !s.is_empty())
                } else {
                    task.assignee.as_deref()
                };
                let _ = state.db.update_task(task_id, title, &task.description, priority, assignee, &task.labels);
            }
            if let Ok(Some(updated)) = state.db.get_task(task_id) {
                let cc = state.db.get_task_comment_counts().unwrap_or_default();
                let td = TaskData {
                    id: updated.id, title: updated.title, description: updated.description,
                    status: updated.status, priority: updated.priority, assignee: updated.assignee,
                    created_by: updated.created_by, created_at: updated.created_at,
                    updated_at: updated.updated_at, position: updated.position,
                    labels: updated.labels,
                    comment_count: *cc.get(&task_id).unwrap_or(&0),
                    project: updated.project,
                };
                let _ = state.broadcast_tx.send(RelayMessage::TaskUpdated { task: td });
            }
        }
    }
}

pub async fn handle_raw_task_comment(
    state: &Arc<RelayState>,
    my_key: &str,
    raw: &serde_json::Value,
) {
    if let (Some(task_id), Some(content)) = (
        raw.get("task_id").and_then(|v| v.as_i64()),
        raw.get("content").and_then(|v| v.as_str()),
    ) {
        let content = content.trim().to_string();
        if content.is_empty() || content.len() > 2000 {
            return;
        }
        let author_name = {
            let peers = state.peers.read().await;
            peers.get(my_key).and_then(|p| p.display_name.clone()).unwrap_or_else(|| my_key[..8].to_string())
        };
        if let Ok(cid) = state.db.add_task_comment(task_id, my_key, &author_name, &content) {
            let msg = serde_json::json!({
                "type": "task_comment_added",
                "task_id": task_id,
                "comment_id": cid,
                "author_name": author_name,
                "content": content,
                "created_at": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
            }).to_string();
            let _ = state.broadcast_tx.send(RelayMessage::System { message: format!("__task_comment__:{}", msg) });
        }
    }
}

pub async fn handle_raw_task_create(
    state: &Arc<RelayState>,
    my_key: &str,
    raw: &serde_json::Value,
) {
    let title = raw.get("title").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if title.is_empty() || title.len() > 200 {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "Task title must be 1-200 characters.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
        return;
    }
    let desc    = raw.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let status  = raw.get("status").and_then(|v| v.as_str()).unwrap_or("backlog");
    let prio    = raw.get("priority").and_then(|v| v.as_str()).unwrap_or("medium");
    let labels  = raw.get("labels").and_then(|v| v.as_str()).unwrap_or("[]");
    let creator_name = {
        let peers = state.peers.read().await;
        peers.get(my_key).and_then(|p| p.display_name.clone()).unwrap_or_else(|| my_key[..8].to_string())
    };
    let valid_statuses  = ["backlog", "in_progress", "testing", "done"];
    let valid_priorities = ["low", "medium", "high", "critical"];
    let status  = if valid_statuses.contains(&status)  { status  } else { "backlog" };
    let prio    = if valid_priorities.contains(&prio)  { prio    } else { "medium"  };
    match state.db.create_task(&title, &desc, status, prio, None, &creator_name, labels) {
        Ok(id) => {
            if let Ok(Some(task)) = state.db.get_task(id) {
                let td = TaskData {
                    id: task.id, title: task.title, description: task.description,
                    status: task.status, priority: task.priority, assignee: task.assignee,
                    created_by: task.created_by, created_at: task.created_at,
                    updated_at: task.updated_at, position: task.position,
                    labels: task.labels, comment_count: 0, project: task.project,
                };
                let _ = state.broadcast_tx.send(RelayMessage::TaskCreated { task: td });
            }
        }
        Err(e) => {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: format!("Failed to create task: {e}"),
            };
            let _ = state.broadcast_tx.send(private);
        }
    }
}

// ── Profile handlers ──

/// Returns true if processing should continue to next message (i.e., caller should `continue`).
pub async fn handle_profile_update(
    state: &Arc<RelayState>,
    my_key: &str,
    last_profile_update: &mut Option<Instant>,
    bio: String,
    socials: String,
    avatar_url: Option<String>,
    banner_url: Option<String>,
    pronouns: Option<String>,
    location: Option<String>,
    website: Option<String>,
    privacy: Option<String>,
) {
    let peer = state.peers.read().await.get(my_key).cloned();
    let display = peer.as_ref().and_then(|p| p.display_name.clone());
    if let Some(ref name) = display {
        // Rate limit: max 1 profile update per 30 seconds.
        let now = Instant::now();
        if let Some(last) = *last_profile_update {
            if now.duration_since(last).as_secs() < 30 {
                let private = RelayMessage::Private {
                    to: my_key.to_string(),
                    message: "⏳ Profile update rate limited. Please wait 30 seconds between updates.".to_string(),
                };
                let _ = state.broadcast_tx.send(private);
                return;
            }
        }

        // Sanitize: strip HTML tags from bio.
        let clean_bio: String = {
            let mut result = String::new();
            let mut in_tag = false;
            for ch in bio.chars() {
                match ch {
                    '<' => in_tag = true,
                    '>' => { in_tag = false; }
                    _ if !in_tag => result.push(ch),
                    _ => {}
                }
            }
            result
        };

        if clean_bio.len() > 280 {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: "Bio too long (max 280 characters).".to_string(),
            };
            let _ = state.broadcast_tx.send(private);
            return;
        }

        if socials.len() > 1024 || serde_json::from_str::<serde_json::Value>(&socials).is_err() {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: "Invalid socials data.".to_string(),
            };
            let _ = state.broadcast_tx.send(private);
            return;
        }

        let is_dangerous = |s: &str| {
            let l = s.trim().to_lowercase();
            l.starts_with("javascript:") || l.starts_with("data:")
        };

        let mut socials_invalid = false;
        if let Ok(socials_obj) = serde_json::from_str::<serde_json::Value>(&socials) {
            if let Some(obj) = socials_obj.as_object() {
                let url_fields = ["website", "youtube"];
                let handle_fields = ["twitter", "github"];

                'socials_check: for field in &url_fields {
                    if let Some(serde_json::Value::String(val)) = obj.get(*field) {
                        let val = val.trim();
                        if val.is_empty() { continue; }
                        if is_dangerous(val) { socials_invalid = true; break 'socials_check; }
                        if !val.starts_with("https://") {
                            let private = RelayMessage::Private {
                                to: my_key.to_string(),
                                message: format!("Profile URL for '{}' must start with https://", field),
                            };
                            let _ = state.broadcast_tx.send(private);
                            socials_invalid = true;
                            break 'socials_check;
                        }
                    }
                }

                if !socials_invalid {
                    'handle_check: for field in &handle_fields {
                        if let Some(serde_json::Value::String(val)) = obj.get(*field) {
                            let val = val.trim();
                            if val.is_empty() { continue; }
                            if !val.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                                let private = RelayMessage::Private {
                                    to: my_key.to_string(),
                                    message: format!("Profile handle for '{}' can only contain letters, numbers, and underscores.", field),
                                };
                                let _ = state.broadcast_tx.send(private);
                                socials_invalid = true;
                                break 'handle_check;
                            }
                        }
                    }
                }

                if !socials_invalid {
                    for (_k, val) in obj {
                        if let Some(s) = val.as_str() {
                            if is_dangerous(s) { socials_invalid = true; break; }
                        }
                    }
                }
            }
        }
        if socials_invalid { return; }

        let avatar  = avatar_url.as_deref().unwrap_or("").trim().to_string();
        let banner  = banner_url.as_deref().unwrap_or("").trim().to_string();
        let w_site  = website.as_deref().unwrap_or("").trim().to_string();
        let pronoun = pronouns.as_deref().unwrap_or("").trim().to_string();
        let loc     = location.as_deref().unwrap_or("").trim().to_string();
        let priv_map = privacy.as_deref().unwrap_or("{}").trim().to_string();

        for url_val in &[&avatar, &banner, &w_site] {
            if url_val.is_empty() { continue; }
            if is_dangerous(url_val) || (!url_val.starts_with("https://")) {
                let private = RelayMessage::Private {
                    to: my_key.to_string(),
                    message: "Profile image/website URLs must start with https://".to_string(),
                };
                let _ = state.broadcast_tx.send(private);
                return;
            }
        }

        if serde_json::from_str::<serde_json::Value>(&priv_map).is_err() {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: "Invalid privacy map.".to_string(),
            };
            let _ = state.broadcast_tx.send(private);
            return;
        }

        match state.db.save_profile_extended(
            name, &clean_bio, &socials, &avatar, &banner, &pronoun, &loc, &w_site, &priv_map,
        ) {
            Ok(()) => {
                *last_profile_update = Some(now);

                let _ = state.broadcast_tx.send(RelayMessage::ProfileData {
                    name: name.clone(),
                    bio: clean_bio.clone(),
                    socials: socials.clone(),
                    avatar_url: if avatar.is_empty() { None } else { Some(avatar.clone()) },
                    banner_url: if banner.is_empty() { None } else { Some(banner.clone()) },
                    pronouns: None,
                    location: None,
                    website: None,
                    target: None,
                });

                let _ = state.broadcast_tx.send(RelayMessage::ProfileData {
                    name: name.clone(),
                    bio: clean_bio,
                    socials,
                    avatar_url: if avatar.is_empty() { None } else { Some(avatar) },
                    banner_url: if banner.is_empty() { None } else { Some(banner) },
                    pronouns: if pronoun.is_empty() { None } else { Some(pronoun) },
                    location: if loc.is_empty() { None } else { Some(loc) },
                    website: if w_site.is_empty() { None } else { Some(w_site) },
                    target: Some(my_key.to_string()),
                });
            }
            Err(e) => {
                tracing::error!("Failed to save profile: {e}");
                let private = RelayMessage::Private {
                    to: my_key.to_string(),
                    message: "Failed to save profile.".to_string(),
                };
                let _ = state.broadcast_tx.send(private);
            }
        }
    } else {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "You must have a registered name to set a profile.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
    }
}

pub async fn handle_profile_request(
    state: &Arc<RelayState>,
    my_key: &str,
    name: String,
) {
    let is_friend = state.db.are_friends(my_key, &{
        let peers = state.peers.read().await;
        peers.values()
            .find(|p| p.display_name.as_deref().map(|n| n.eq_ignore_ascii_case(&name)).unwrap_or(false))
            .map(|p| p.public_key_hex.clone())
            .unwrap_or_default()
    }).unwrap_or(false);

    match state.db.get_public_profile(&name, is_friend) {
        Ok(Some(fields)) => {
            let get = |k: &str| fields.get(k).cloned().filter(|v| !v.is_empty());
            let _ = state.broadcast_tx.send(RelayMessage::ProfileData {
                name: name.clone(),
                bio: fields.get("bio").cloned().unwrap_or_default(),
                socials: fields.get("socials").cloned().unwrap_or_else(|| "{}".to_string()),
                avatar_url: get("avatar_url"),
                banner_url: get("banner_url"),
                pronouns:   get("pronouns"),
                location:   get("location"),
                website:    get("website"),
                target: Some(my_key.to_string()),
            });
        }
        Ok(None) => {
            let _ = state.broadcast_tx.send(RelayMessage::ProfileData {
                name: name.clone(),
                bio: String::new(),
                socials: "{}".to_string(),
                avatar_url: None,
                banner_url: None,
                pronouns:   None,
                location:   None,
                website:    None,
                target: Some(my_key.to_string()),
            });
        }
        Err(e) => {
            tracing::error!("Failed to get profile: {e}");
        }
    }
}

// ── DM handlers ──

pub async fn handle_dm(
    state: &Arc<RelayState>,
    my_key: &str,
    to: String,
    content: String,
    encrypted: bool,
    nonce: Option<String>,
) {
    let peer = state.peers.read().await.get(my_key).cloned();
    let sender_name = peer.as_ref()
        .and_then(|p| p.display_name.clone())
        .unwrap_or_else(|| "Anonymous".to_string());

    if content.is_empty() {
        return;
    }
    let dm_role = state.db.get_role(my_key).unwrap_or_default();
    let dm_char_limit: usize = if dm_role == "admin" { 10_000 } else { 2_000 };
    if content.len() > dm_char_limit {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: format!("DM too long (max {} chars).", dm_char_limit),
        };
        let _ = state.broadcast_tx.send(private);
        return;
    }
    if to == my_key {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "You can't DM yourself.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
        return;
    }
    let user_role = state.db.get_role(my_key).unwrap_or_default();
    if user_role != "admin" && user_role != "mod" && !my_key.starts_with("bot_") {
        if user_role != "verified" && user_role != "donor" {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: "🔒 Verify your account to send DMs.".to_string(),
            };
            let _ = state.broadcast_tx.send(private);
            return;
        }
        let are_friends = state.db.are_friends(my_key, &to).unwrap_or(false);
        if !are_friends {
            let target_name = state.db.name_for_key(&to).ok().flatten().unwrap_or_else(|| "this user".to_string());
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: format!("🔒 You must be friends to DM {target_name}. Use /follow <name> — if they follow you back, you'll be friends."),
            };
            let _ = state.broadcast_tx.send(private);
            return;
        }
    }

    if user_role == "muted" {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "You are muted and cannot send DMs.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
        return;
    }

    // Fibonacci rate limiting for DMs (skip for bots and admins).
    if !my_key.starts_with("bot_") && user_role != "admin" {
        let now = Instant::now();
        let mut rate_limits = state.rate_limits.write().await;
        let rl = rate_limits.entry(my_key.to_string()).or_insert_with(|| {
            let unix_now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default().as_secs();
            let reg_at = {
                let conn = state.db.conn.lock().unwrap();
                conn.query_row(
                    "SELECT MIN(registered_at) FROM registered_names WHERE public_key = ?1",
                    rusqlite::params![my_key],
                    |row| row.get::<_, Option<i64>>(0),
                ).ok().flatten().unwrap_or(unix_now as i64) as u64
            };
            let account_age = unix_now.saturating_sub(reg_at);
            let first_seen = if account_age >= NEW_ACCOUNT_WINDOW_SECS {
                now - std::time::Duration::from_secs(NEW_ACCOUNT_WINDOW_SECS + 1)
            } else {
                now - std::time::Duration::from_secs(account_age)
            };
            RateLimitState {
                first_seen,
                last_message_time: now - std::time::Duration::from_secs(60),
                fib_index: 0,
            }
        });

        let elapsed = now.duration_since(rl.last_message_time).as_secs();
        let fib_delay = FIB_DELAYS[rl.fib_index];

        let is_trusted = user_role == "verified" || user_role == "donor" || user_role == "mod" || user_role == "admin";
        let account_age = now.duration_since(rl.first_seen).as_secs();
        let new_account_delay = if !is_trusted && account_age < NEW_ACCOUNT_WINDOW_SECS {
            NEW_ACCOUNT_DELAY_SECS
        } else {
            0
        };

        let required_delay = fib_delay.max(new_account_delay);

        if elapsed < required_delay {
            let wait = required_delay - elapsed;
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: format!("⏳ Slow down! Please wait {} more second{}.", wait, if wait == 1 { "" } else { "s" }),
            };
            let _ = state.broadcast_tx.send(private);
            return;
        }

        if elapsed > required_delay {
            rl.fib_index = 0;
        } else {
            rl.fib_index = (rl.fib_index + 1).min(FIB_DELAYS.len() - 1);
        }

        rl.last_message_time = now;
    }

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    if let Err(e) = state.db.store_dm_e2ee(my_key, &sender_name, &to, &content, ts, encrypted, nonce.as_deref()) {
        tracing::error!("Failed to store DM: {e}");
    }

    let dm_msg = RelayMessage::Dm {
        from: my_key.to_string(),
        from_name: Some(sender_name.clone()),
        to: to.clone(),
        content: content.clone(),
        timestamp: ts,
        encrypted,
        nonce: nonce.clone(),
    };
    let _ = state.broadcast_tx.send(dm_msg);

    let target_online = state.peers.read().await.contains_key(&to);
    if !target_online {
        let push_body = if encrypted {
            "New encrypted message".to_string()
        } else {
            let max = 100.min(content.len());
            content[..max].to_string()
        };
        let tag = format!("dm-{}", &my_key[..8.min(my_key.len())]);
        state.send_push_notification(
            &to,
            &format!("DM from {}", sender_name),
            &push_body,
            &tag,
            "/chat",
        );
    }

    send_dm_list_update(state, my_key);
    send_dm_list_update(state, &to);
}

pub async fn handle_dm_open(
    state: &Arc<RelayState>,
    my_key: &str,
    partner: String,
) {
    let my_name = state.db.name_for_key(my_key).ok().flatten();
    let partner_name = state.db.name_for_key(&partner).ok().flatten();

    if let (Some(pn), Some(mn)) = (&partner_name, &my_name) {
        let _ = state.db.mark_dms_read_by_name(pn, mn);
    } else {
        let _ = state.db.mark_dms_read(&partner, my_key);
    }

    let records = if let (Some(mn), Some(pn)) = (&my_name, &partner_name) {
        state.db.load_dm_conversation_by_name(mn, pn, 100)
    } else {
        state.db.load_dm_conversation(my_key, &partner, 100)
    };

    match records {
        Ok(records) => {
            let messages: Vec<DmData> = records.into_iter().map(|r| DmData {
                from: r.from_key,
                from_name: r.from_name,
                to: r.to_key,
                content: r.content,
                timestamp: r.timestamp,
                encrypted: r.encrypted,
                nonce: r.nonce,
            }).collect();
            let history = RelayMessage::DmHistory {
                target: Some(my_key.to_string()),
                partner,
                messages,
            };
            let _ = state.broadcast_tx.send(history);
        }
        Err(e) => {
            tracing::error!("Failed to load DM history: {e}");
        }
    }
    send_dm_list_update(state, my_key);
}

pub async fn handle_dm_read(
    state: &Arc<RelayState>,
    my_key: &str,
    partner: String,
) {
    let my_name = state.db.name_for_key(my_key).ok().flatten();
    let partner_name = state.db.name_for_key(&partner).ok().flatten();
    if let (Some(pn), Some(mn)) = (&partner_name, &my_name) {
        let _ = state.db.mark_dms_read_by_name(pn, mn);
    } else {
        let _ = state.db.mark_dms_read(&partner, my_key);
    }
    send_dm_list_update(state, my_key);
}

// ── Voice handlers ──

pub async fn handle_voice_call(
    state: &Arc<RelayState>,
    my_key: &str,
    to: String,
    action: String,
) {
    let peer = state.peers.read().await.get(my_key).cloned();
    let sender_name = peer.as_ref()
        .and_then(|p| p.display_name.clone());
    let target_connected = state.peers.read().await.contains_key(&to);
    if !target_connected {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "User is not online.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
    } else {
        let msg = RelayMessage::VoiceCall {
            from: my_key.to_string(),
            from_name: sender_name,
            to,
            action,
        };
        let _ = state.broadcast_tx.send(msg);
    }
}

pub async fn handle_webrtc_signal(
    state: &Arc<RelayState>,
    my_key: &str,
    to: String,
    signal_type: String,
    data: serde_json::Value,
) {
    let target_connected = state.peers.read().await.contains_key(&to);
    if !target_connected {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "User is not online.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
    } else {
        let msg = RelayMessage::WebrtcSignal {
            from: my_key.to_string(),
            to,
            signal_type,
            data,
        };
        let _ = state.broadcast_tx.send(msg);
    }
}

pub async fn handle_voice_room(
    state: &Arc<RelayState>,
    my_key: &str,
    action: String,
    room_id: Option<String>,
    room_name: Option<String>,
) {
    let peer = state.peers.read().await.get(my_key).cloned();
    let display = peer.as_ref().and_then(|p| p.display_name.clone()).unwrap_or_else(|| "Anonymous".to_string());
    let user_role = state.db.get_role(my_key).unwrap_or_default();
    match action.as_str() {
        "create" => {
            if user_role != "admin" && user_role != "mod" {
                let private = RelayMessage::Private { to: my_key.to_string(), message: "Only admins and mods can create voice channels.".to_string() };
                let _ = state.broadcast_tx.send(private);
            } else {
                let rname = room_name.unwrap_or_else(|| format!("{}'s Room", display));
                match state.db.create_voice_channel(&rname, my_key) {
                    Ok(_id) => {
                        broadcast_voice_channel_list(state).await;
                    }
                    Err(e) => {
                        tracing::error!("Failed to create voice channel: {e}");
                    }
                }
            }
        }
        "join" => {
            if let Some(rid) = room_id {
                let id_num: i64 = rid.parse().unwrap_or(0);
                if id_num == 0 || !state.db.voice_channel_exists(id_num).unwrap_or(false) {
                    let private = RelayMessage::Private { to: my_key.to_string(), message: "Voice channel not found.".to_string() };
                    let _ = state.broadcast_tx.send(private);
                } else {
                    let mut rooms = state.voice_rooms.write().await;
                    let room = rooms.entry(rid.clone()).or_insert_with(|| {
                        let name = state.db.list_voice_channels().ok()
                            .and_then(|chs| chs.into_iter().find(|c| c.id == id_num).map(|c| c.name))
                            .unwrap_or_else(|| "Voice Channel".to_string());
                        VoiceRoom { name, participants: vec![] }
                    });
                    if !room.participants.iter().any(|(k, _)| k == my_key) {
                        let existing: Vec<(String, String)> = room.participants.clone();
                        room.participants.push((my_key.to_string(), display.clone()));
                        drop(rooms);
                        for (pk, _) in &existing {
                            let _ = state.broadcast_tx.send(RelayMessage::VoiceRoomSignal {
                                from: my_key.to_string(),
                                to: pk.clone(),
                                room_id: rid.clone(),
                                signal_type: "new_participant".to_string(),
                                data: serde_json::json!({ "key": my_key, "name": display }),
                            });
                        }
                        broadcast_voice_channel_list(state).await;
                    } else {
                        drop(rooms);
                    }
                }
            }
        }
        "leave" => {
            leave_voice_room(state, my_key).await;
        }
        "rename" => {
            if user_role != "admin" && user_role != "mod" {
                let private = RelayMessage::Private { to: my_key.to_string(), message: "Only admins and mods can rename voice channels.".to_string() };
                let _ = state.broadcast_tx.send(private);
            } else if let Some(rid) = room_id {
                let id_num: i64 = rid.parse().unwrap_or(0);
                let new_name = room_name.unwrap_or_default().trim().to_string();
                if id_num <= 0 {
                    let private = RelayMessage::Private { to: my_key.to_string(), message: "Voice channel not found.".to_string() };
                    let _ = state.broadcast_tx.send(private);
                } else if new_name.is_empty() || new_name.len() > 48 {
                    let private = RelayMessage::Private { to: my_key.to_string(), message: "Voice channel name must be 1-48 characters.".to_string() };
                    let _ = state.broadcast_tx.send(private);
                } else {
                    match state.db.rename_voice_channel(id_num, &new_name) {
                        Ok(true) => {
                            if let Some(room) = state.voice_rooms.write().await.get_mut(&rid) {
                                room.name = new_name.clone();
                            }
                            broadcast_voice_channel_list(state).await;
                        }
                        Ok(false) => {
                            let private = RelayMessage::Private { to: my_key.to_string(), message: "Voice channel not found.".to_string() };
                            let _ = state.broadcast_tx.send(private);
                        }
                        Err(e) => tracing::error!("Failed to rename voice channel: {e}"),
                    }
                }
            }
        }
        "delete" => {
            if user_role != "admin" && user_role != "mod" {
                let private = RelayMessage::Private { to: my_key.to_string(), message: "Only admins and mods can delete voice channels.".to_string() };
                let _ = state.broadcast_tx.send(private);
            } else if let Some(rid) = room_id {
                let id_num: i64 = rid.parse().unwrap_or(0);
                if id_num > 0 {
                    let _ = state.db.delete_voice_channel(id_num);
                    state.voice_rooms.write().await.remove(&rid);
                    broadcast_voice_channel_list(state).await;
                }
            }
        }
        "list" => {
            broadcast_voice_channel_list(state).await;
        }
        _ => {}
    }
}

pub async fn handle_voice_room_signal(
    state: &Arc<RelayState>,
    my_key: &str,
    to: String,
    room_id: String,
    signal_type: String,
    data: serde_json::Value,
) {
    let rooms = state.voice_rooms.read().await;
    let valid = rooms.get(&room_id).map(|r| {
        r.participants.iter().any(|(k, _)| k == my_key) &&
        r.participants.iter().any(|(k, _)| k == &to)
    }).unwrap_or(false);
    drop(rooms);
    if valid {
        let _ = state.broadcast_tx.send(RelayMessage::VoiceRoomSignal {
            from: my_key.to_string(),
            to,
            room_id,
            signal_type,
            data,
        });
    }
}

// ── Task handlers (RelayMessage enum variants) ──

pub async fn handle_task_list(
    state: &Arc<RelayState>,
    my_key: &str,
) {
    let tasks = build_task_list(&state.db);
    let _ = state.broadcast_tx.send(RelayMessage::TaskListResponse {
        target: Some(my_key.to_string()),
        tasks,
    });
}

pub async fn handle_task_create(
    state: &Arc<RelayState>,
    my_key: &str,
    title: String,
    description: String,
    status: String,
    priority: String,
    assignee: Option<String>,
    labels: String,
    project: String,
) {
    let role = state.db.get_role(my_key).unwrap_or_default();
    if role != "admin" && role != "mod" {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "Only admins and mods can create tasks.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
    } else {
        let valid_statuses = ["backlog", "in_progress", "testing", "done"];
        let valid_priorities = ["low", "medium", "high", "critical"];
        let s = if valid_statuses.contains(&status.as_str()) { &status } else { "backlog" };
        let p = if valid_priorities.contains(&priority.as_str()) { &priority } else { "medium" };
        let proj = if project.is_empty() { "default" } else { &project };
        if title.trim().is_empty() || title.len() > 200 {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: "Task title must be 1-200 characters.".to_string(),
            };
            let _ = state.broadcast_tx.send(private);
        } else if description.len() > 5000 {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: "Task description too long (max 5000 chars).".to_string(),
            };
            let _ = state.broadcast_tx.send(private);
        } else {
            match state.db.create_task_in_project(&title, &description, s, p, assignee.as_deref(), my_key, &labels, proj) {
                Ok(id) => {
                    if let Ok(Some(task)) = state.db.get_task(id) {
                        let td = TaskData {
                            id: task.id, title: task.title, description: task.description,
                            status: task.status, priority: task.priority, assignee: task.assignee,
                            created_by: task.created_by, created_at: task.created_at,
                            updated_at: task.updated_at, position: task.position, labels: task.labels,
                            comment_count: 0, project: task.project,
                        };
                        let _ = state.broadcast_tx.send(RelayMessage::TaskCreated { task: td });
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to create task: {e}");
                }
            }
        }
    }
}

pub async fn handle_task_update_msg(
    state: &Arc<RelayState>,
    my_key: &str,
    id: i64,
    title: String,
    description: String,
    priority: String,
    assignee: Option<String>,
    labels: String,
    project: String,
) {
    let role = state.db.get_role(my_key).unwrap_or_default();
    if role != "admin" && role != "mod" {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "Only admins and mods can edit tasks.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
    } else {
        let valid_priorities = ["low", "medium", "high", "critical"];
        let p = if valid_priorities.contains(&priority.as_str()) { &priority } else { "medium" };
        let proj = if project.is_empty() { "default" } else { &project };
        match state.db.update_task_with_project(id, &title, &description, p, assignee.as_deref(), &labels, proj) {
            Ok(true) => {
                if let Ok(Some(task)) = state.db.get_task(id) {
                    let cc = state.db.get_task_comment_counts().unwrap_or_default();
                    let td = TaskData {
                        id: task.id, title: task.title, description: task.description,
                        status: task.status, priority: task.priority, assignee: task.assignee,
                        created_by: task.created_by, created_at: task.created_at,
                        updated_at: task.updated_at, position: task.position, labels: task.labels,
                        comment_count: *cc.get(&task.id).unwrap_or(&0), project: task.project,
                    };
                    let _ = state.broadcast_tx.send(RelayMessage::TaskUpdated { task: td });
                }
            }
            Ok(false) => {
                let private = RelayMessage::Private {
                    to: my_key.to_string(),
                    message: "Task not found.".to_string(),
                };
                let _ = state.broadcast_tx.send(private);
            }
            Err(e) => tracing::error!("Task update error: {e}"),
        }
    }
}

pub async fn handle_task_move(
    state: &Arc<RelayState>,
    my_key: &str,
    id: i64,
    status: String,
) {
    let role = state.db.get_role(my_key).unwrap_or_default();
    if role != "admin" && role != "mod" {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "Only admins and mods can move tasks.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
    } else {
        let valid_statuses = ["backlog", "in_progress", "testing", "done"];
        if !valid_statuses.contains(&status.as_str()) {
            return;
        }
        match state.db.move_task(id, &status) {
            Ok(true) => {
                let _ = state.broadcast_tx.send(RelayMessage::TaskMoved { id, status });
            }
            Ok(false) => {
                let private = RelayMessage::Private {
                    to: my_key.to_string(),
                    message: "Task not found.".to_string(),
                };
                let _ = state.broadcast_tx.send(private);
            }
            Err(e) => tracing::error!("Task move error: {e}"),
        }
    }
}

pub async fn handle_task_delete(
    state: &Arc<RelayState>,
    my_key: &str,
    id: i64,
) {
    let role = state.db.get_role(my_key).unwrap_or_default();
    if role != "admin" && role != "mod" {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "Only admins and mods can delete tasks.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
    } else {
        match state.db.delete_task(id) {
            Ok(true) => {
                let _ = state.broadcast_tx.send(RelayMessage::TaskDeleted { id });
            }
            Ok(false) => {
                let private = RelayMessage::Private {
                    to: my_key.to_string(),
                    message: "Task not found.".to_string(),
                };
                let _ = state.broadcast_tx.send(private);
            }
            Err(e) => tracing::error!("Task delete error: {e}"),
        }
    }
}

pub async fn handle_task_comment_msg(
    state: &Arc<RelayState>,
    my_key: &str,
    task_id: i64,
    content: String,
) {
    let role = state.db.get_role(my_key).unwrap_or_default();
    let can_comment = role == "admin" || role == "mod" || role == "verified" || role == "donor";
    if !can_comment {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "Only verified users can comment on tasks.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
    } else if content.trim().is_empty() || content.len() > 2000 {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "Comment must be 1-2000 characters.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
    } else {
        let peer = state.peers.read().await.get(my_key).cloned();
        let author_name = peer.as_ref()
            .and_then(|p| p.display_name.clone())
            .unwrap_or_else(|| "Anonymous".to_string());
        match state.db.add_task_comment(task_id, my_key, &author_name, &content) {
            Ok(comment_id) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64;
                let comment = TaskCommentData {
                    id: comment_id,
                    task_id,
                    author_key: my_key.to_string(),
                    author_name,
                    content,
                    created_at: now,
                };
                let _ = state.broadcast_tx.send(RelayMessage::TaskCommentAdded { task_id, comment });
            }
            Err(e) => tracing::error!("Task comment error: {e}"),
        }
    }
}

pub async fn handle_task_comments_request(
    state: &Arc<RelayState>,
    my_key: &str,
    task_id: i64,
) {
    match state.db.get_task_comments(task_id) {
        Ok(records) => {
            let comments: Vec<TaskCommentData> = records.into_iter().map(|r| TaskCommentData {
                id: r.id, task_id: r.task_id, author_key: r.author_key,
                author_name: r.author_name, content: r.content, created_at: r.created_at,
            }).collect();
            let _ = state.broadcast_tx.send(RelayMessage::TaskCommentsResponse {
                target: Some(my_key.to_string()),
                task_id,
                comments,
            });
        }
        Err(e) => tracing::error!("Task comments load error: {e}"),
    }
}

// ── Social handlers ──

pub async fn handle_follow(
    state: &Arc<RelayState>,
    my_key: &str,
    target_key: String,
) {
    if target_key == my_key {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "You can't follow yourself.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
        return;
    }
    let target_name = state.db.name_for_key(&target_key).ok().flatten();
    if target_name.is_none() {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "User not found.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
        return;
    }
    match state.db.add_follow(my_key, &target_key) {
        Ok(true) => {
            let my_name = state.db.name_for_key(my_key).ok().flatten().unwrap_or_else(|| "Someone".to_string());
            let tname = target_name.unwrap();
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: format!("✅ You are now following {tname}."),
            };
            let _ = state.broadcast_tx.send(private);
            let _ = state.broadcast_tx.send(RelayMessage::FollowUpdate {
                follower_key: my_key.to_string(),
                followed_key: target_key.clone(),
                action: "follow".to_string(),
            });
            let private2 = RelayMessage::Private {
                to: target_key.clone(),
                message: format!("👁️ {my_name} is now following you."),
            };
            let _ = state.broadcast_tx.send(private2);
        }
        Ok(false) => {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: "You are already following this user.".to_string(),
            };
            let _ = state.broadcast_tx.send(private);
        }
        Err(e) => tracing::error!("Follow error: {e}"),
    }
}

pub async fn handle_unfollow(
    state: &Arc<RelayState>,
    my_key: &str,
    target_key: String,
) {
    match state.db.remove_follow(my_key, &target_key) {
        Ok(true) => {
            let tname = state.db.name_for_key(&target_key).ok().flatten().unwrap_or_else(|| "user".to_string());
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: format!("✅ You unfollowed {tname}."),
            };
            let _ = state.broadcast_tx.send(private);
            let _ = state.broadcast_tx.send(RelayMessage::FollowUpdate {
                follower_key: my_key.to_string(),
                followed_key: target_key,
                action: "unfollow".to_string(),
            });
        }
        Ok(false) => {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: "You are not following this user.".to_string(),
            };
            let _ = state.broadcast_tx.send(private);
        }
        Err(e) => tracing::error!("Unfollow error: {e}"),
    }
}

pub async fn handle_friend_code_request(
    state: &Arc<RelayState>,
    my_key: &str,
) {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let expires = now_ms + 24 * 60 * 60 * 1000;
    match state.db.create_friend_code(my_key, expires, 1) {
        Ok(code) => {
            let _ = state.broadcast_tx.send(RelayMessage::FriendCodeResponse {
                code,
                target: Some(my_key.to_string()),
            });
        }
        Err(e) => {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: format!("❌ {e}"),
            };
            let _ = state.broadcast_tx.send(private);
        }
    }
}

pub async fn handle_friend_code_redeem(
    state: &Arc<RelayState>,
    my_key: &str,
    code: String,
) {
    match state.db.redeem_friend_code(&code) {
        Ok(Some((owner_key, owner_name))) => {
            if owner_key == my_key {
                let _ = state.broadcast_tx.send(RelayMessage::FriendCodeResult {
                    success: false,
                    name: None,
                    message: "You can't redeem your own friend code.".to_string(),
                    target: Some(my_key.to_string()),
                });
                return;
            }
            let my_name = state.db.name_for_key(my_key).ok().flatten().unwrap_or_else(|| "Someone".to_string());
            let oname = owner_name.clone().unwrap_or_else(|| "Unknown".to_string());

            let _ = state.db.add_follow(my_key, &owner_key);
            let _ = state.db.add_follow(&owner_key, my_key);

            let _ = state.broadcast_tx.send(RelayMessage::FollowUpdate {
                follower_key: my_key.to_string(),
                followed_key: owner_key.clone(),
                action: "follow".to_string(),
            });
            let _ = state.broadcast_tx.send(RelayMessage::FollowUpdate {
                follower_key: owner_key.clone(),
                followed_key: my_key.to_string(),
                action: "follow".to_string(),
            });

            let _ = state.broadcast_tx.send(RelayMessage::FriendCodeResult {
                success: true,
                name: owner_name.clone(),
                message: format!("🎉 You are now friends with {oname}!"),
                target: Some(my_key.to_string()),
            });

            let private = RelayMessage::Private {
                to: owner_key.clone(),
                message: format!("🎉 {my_name} redeemed your friend code! You are now friends."),
            };
            let _ = state.broadcast_tx.send(private);
        }
        Ok(None) => {
            let _ = state.broadcast_tx.send(RelayMessage::FriendCodeResult {
                success: false,
                name: None,
                message: "Invalid or expired friend code.".to_string(),
                target: Some(my_key.to_string()),
            });
        }
        Err(e) => {
            tracing::error!("Friend code redeem error: {e}");
            let _ = state.broadcast_tx.send(RelayMessage::FriendCodeResult {
                success: false,
                name: None,
                message: "Server error while redeeming code.".to_string(),
                target: Some(my_key.to_string()),
            });
        }
    }
}

// ── Group handlers ──

pub async fn handle_group_create(
    state: &Arc<RelayState>,
    my_key: &str,
    name: String,
) {
    let user_role = state.db.get_role(my_key).unwrap_or_default();
    if user_role != "verified" && user_role != "donor" && user_role != "mod" && user_role != "admin" {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "You must be verified to create groups.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
        return;
    }
    if name.trim().is_empty() || name.len() > 50 {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "Group name must be 1-50 characters.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
        return;
    }
    match state.db.create_group(name.trim(), my_key) {
        Ok((id, invite_code)) => {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: format!("✅ Group '{}' created! Invite code: {invite_code}", name.trim()),
            };
            let _ = state.broadcast_tx.send(private);
            if let Ok(user_groups) = state.db.get_user_groups(my_key) {
                let groups: Vec<GroupData> = user_groups.into_iter().map(|(id, name, invite_code, role)| {
                    GroupData { id, name, invite_code, role }
                }).collect();
                let _ = state.broadcast_tx.send(RelayMessage::GroupList {
                    target: Some(my_key.to_string()),
                    groups,
                });
            }
        }
        Err(e) => tracing::error!("Group create error: {e}"),
    }
}

pub async fn handle_group_join(
    state: &Arc<RelayState>,
    my_key: &str,
    invite_code: String,
) {
    match state.db.join_group_by_invite(&invite_code, my_key) {
        Ok(Some((gid, gname))) => {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: format!("✅ Joined group '{gname}'."),
            };
            let _ = state.broadcast_tx.send(private);
            if let Ok(user_groups) = state.db.get_user_groups(my_key) {
                let groups: Vec<GroupData> = user_groups.into_iter().map(|(id, name, invite_code, role)| {
                    GroupData { id, name, invite_code, role }
                }).collect();
                let _ = state.broadcast_tx.send(RelayMessage::GroupList {
                    target: Some(my_key.to_string()),
                    groups,
                });
            }
        }
        Ok(None) => {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: "Invalid invite code.".to_string(),
            };
            let _ = state.broadcast_tx.send(private);
        }
        Err(e) => tracing::error!("Group join error: {e}"),
    }
}

pub async fn handle_group_leave(
    state: &Arc<RelayState>,
    my_key: &str,
    group_id: String,
) {
    match state.db.leave_group(&group_id, my_key) {
        Ok(true) => {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: "✅ Left the group.".to_string(),
            };
            let _ = state.broadcast_tx.send(private);
            if let Ok(user_groups) = state.db.get_user_groups(my_key) {
                let groups: Vec<GroupData> = user_groups.into_iter().map(|(id, name, invite_code, role)| {
                    GroupData { id, name, invite_code, role }
                }).collect();
                let _ = state.broadcast_tx.send(RelayMessage::GroupList {
                    target: Some(my_key.to_string()),
                    groups,
                });
            }
        }
        Ok(false) => {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: "You are not in this group.".to_string(),
            };
            let _ = state.broadcast_tx.send(private);
        }
        Err(e) => tracing::error!("Group leave error: {e}"),
    }
}

pub async fn handle_group_history_request(
    state: &Arc<RelayState>,
    my_key: &str,
    group_id: String,
) {
    let is_member = state.db.is_group_member(&group_id, my_key).unwrap_or(false);
    if !is_member {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "You are not a member of this group.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
        return;
    }

    match state.db.load_group_messages(&group_id, 200) {
        Ok(rows) => {
            let messages: Vec<GroupMessageData> = rows
                .into_iter()
                .map(|(from, from_name, content, timestamp)| GroupMessageData {
                    from, from_name, content, timestamp,
                })
                .collect();
            let _ = state.broadcast_tx.send(RelayMessage::GroupHistory {
                target: Some(my_key.to_string()),
                group_id,
                messages,
            });
        }
        Err(e) => tracing::error!("Group history error: {e}"),
    }
}

pub async fn handle_group_members_request(
    state: &Arc<RelayState>,
    my_key: &str,
    group_id: String,
) {
    let is_member = state.db.is_group_member(&group_id, my_key).unwrap_or(false);
    if !is_member {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "You are not a member of this group.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
        return;
    }

    match state.db.get_group_members(&group_id) {
        Ok(members) => {
            let _ = state.broadcast_tx.send(RelayMessage::GroupMembers {
                target: Some(my_key.to_string()),
                group_id,
                members,
            });
        }
        Err(e) => tracing::error!("Group members error: {e}"),
    }
}

pub async fn handle_group_msg(
    state: &Arc<RelayState>,
    my_key: &str,
    group_id: String,
    content: String,
) {
    let is_member = state.db.is_group_member(&group_id, my_key).unwrap_or(false);
    if !is_member {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "You are not a member of this group.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
        return;
    }
    if content.is_empty() || content.len() > 2000 {
        return;
    }
    let sender_name = state.db.name_for_key(my_key).ok().flatten().unwrap_or_else(|| "Anonymous".to_string());
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let _ = state.db.store_group_message(&group_id, my_key, &sender_name, &content, ts);

    if let Ok(members) = state.db.get_group_members(&group_id) {
        for (member_key, _role) in members {
            let gm = RelayMessage::GroupMessage {
                group_id: group_id.clone(),
                from: my_key.to_string(),
                from_name: Some(sender_name.clone()),
                content: content.clone(),
                timestamp: ts,
                target: Some(member_key),
            };
            let _ = state.broadcast_tx.send(gm);
        }
    }
}

// ── Server Membership handlers ──

pub async fn handle_member_list_request(
    state: &Arc<RelayState>,
    my_key: &str,
    limit: Option<usize>,
    offset: Option<usize>,
    search: Option<String>,
) {
    let limit = limit.unwrap_or(50).min(200);
    let offset = offset.unwrap_or(0);
    let search_ref = search.as_deref();
    let members = state.db.get_members(limit, offset, search_ref).unwrap_or_default();
    let total = state.db.get_member_count(search_ref).unwrap_or(0);
    let data: Vec<MemberData> = members.into_iter().map(|m| MemberData {
        public_key: m.public_key,
        name: m.name,
        role: m.role,
        joined_at: m.joined_at,
        last_seen: m.last_seen,
    }).collect();
    let _ = state.broadcast_tx.send(RelayMessage::MemberListResponse {
        target: Some(my_key.to_string()),
        members: data,
        total,
    });
}

// ── Marketplace handlers ──

pub async fn handle_listing_browse(
    state: &Arc<RelayState>,
    my_key: &str,
) {
    let listings = state.db.get_listings(None, None, 200).unwrap_or_default();
    let data: Vec<ListingData> = listings.iter().map(listing_from_db).collect();
    let _ = state.broadcast_tx.send(RelayMessage::ListingList {
        target: Some(my_key.to_string()),
        listings: data,
    });
}

pub async fn handle_listing_create(
    state: &Arc<RelayState>,
    my_key: &str,
    id: String,
    title: String,
    description: String,
    category: String,
    condition: String,
    price: String,
    payment_methods: String,
    location: String,
) {
    let user_role = state.db.get_role(my_key).unwrap_or_default();
    if user_role != "verified" && user_role != "donor" && user_role != "mod" && user_role != "admin" {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "You must be verified to create listings.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
        return;
    }
    if title.trim().is_empty() || title.len() > 100 {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "Listing title must be 1-100 characters.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
        return;
    }
    let seller_name = state.db.name_for_key(my_key).ok().flatten().unwrap_or_else(|| "Anonymous".to_string());
    if let Err(e) = state.db.create_listing(&id, my_key, &seller_name, title.trim(), &description, &category, &condition, &price, &payment_methods, &location) {
        tracing::error!("Failed to create listing: {e}");
        return;
    }
    if let Ok(Some(listing)) = state.db.get_listing_by_id(&id) {
        let _ = state.broadcast_tx.send(RelayMessage::ListingNew {
            listing: listing_from_db(&listing),
        });
    }
}

pub async fn handle_listing_update(
    state: &Arc<RelayState>,
    my_key: &str,
    id: String,
    title: String,
    description: String,
    category: String,
    condition: String,
    price: String,
    payment_methods: String,
    location: String,
    status: Option<String>,
) {
    let user_role = state.db.get_role(my_key).unwrap_or_default();
    let is_admin = user_role == "admin" || user_role == "mod";
    if let Ok(true) = state.db.update_listing(&id, my_key, title.trim(), &description, &category, &condition, &price, &payment_methods, &location, status.as_deref(), is_admin) {
        if let Ok(Some(listing)) = state.db.get_listing_by_id(&id) {
            let _ = state.broadcast_tx.send(RelayMessage::ListingUpdated {
                listing: listing_from_db(&listing),
            });
        }
    }
}

pub async fn handle_listing_delete(
    state: &Arc<RelayState>,
    my_key: &str,
    id: String,
) {
    let user_role = state.db.get_role(my_key).unwrap_or_default();
    let is_admin = user_role == "admin" || user_role == "mod";
    if let Ok(true) = state.db.delete_listing(&id, my_key, is_admin) {
        let _ = state.broadcast_tx.send(RelayMessage::ListingDeleted { id });
    }
}

// ── Review handlers ──

pub async fn handle_review_create(
    state: &Arc<RelayState>,
    my_key: &str,
    listing_id: String,
    rating: i32,
    comment: String,
) {
    // Validate rating range.
    if !(1..=5).contains(&rating) {
        let _ = state.broadcast_tx.send(RelayMessage::Private {
            to: my_key.to_string(),
            message: "Rating must be between 1 and 5.".to_string(),
        });
        return;
    }

    // Limit comment length.
    if comment.len() > 2000 {
        let _ = state.broadcast_tx.send(RelayMessage::Private {
            to: my_key.to_string(),
            message: "Review comment must be under 2000 characters.".to_string(),
        });
        return;
    }

    let reviewer_name = state.db.name_for_key(my_key).ok().flatten().unwrap_or_else(|| "Anonymous".to_string());

    match state.db.create_review(&listing_id, my_key, &reviewer_name, rating, &comment) {
        Ok(review_id) => {
            if let Ok(Some(review)) = state.db.get_review_by_id(review_id) {
                let _ = state.broadcast_tx.send(RelayMessage::ReviewCreated {
                    review: review_from_db(&review),
                });
            }
        }
        Err(e) => {
            let _ = state.broadcast_tx.send(RelayMessage::Private {
                to: my_key.to_string(),
                message: e,
            });
        }
    }
}

pub async fn handle_review_delete(
    state: &Arc<RelayState>,
    my_key: &str,
    listing_id: String,
    review_id: i64,
) {
    let user_role = state.db.get_role(my_key).unwrap_or_default();
    let is_admin = user_role == "admin" || user_role == "mod";

    match state.db.delete_review(review_id, my_key, is_admin) {
        Ok(true) => {
            let _ = state.broadcast_tx.send(RelayMessage::ReviewDeleted {
                listing_id,
                review_id,
            });
        }
        Ok(false) => {
            let _ = state.broadcast_tx.send(RelayMessage::Private {
                to: my_key.to_string(),
                message: "Review not found.".to_string(),
            });
        }
        Err(e) => {
            let _ = state.broadcast_tx.send(RelayMessage::Private {
                to: my_key.to_string(),
                message: e,
            });
        }
    }
}

// ── Device handlers ──

pub async fn handle_device_list_request(
    state: &Arc<RelayState>,
    my_key: &str,
) {
    if let Ok(Some(name)) = state.db.name_for_key(my_key) {
        match state.db.keys_for_name_detailed(&name) {
            Ok(keys) => {
                let peers = state.peers.read().await;
                let devices: Vec<DeviceInfo> = keys.into_iter().map(|(key, label, reg_at)| {
                    let is_online = peers.values().any(|p| p.public_key_hex == key);
                    DeviceInfo {
                        is_current: key == my_key,
                        public_key: key,
                        label,
                        registered_at: reg_at as u64,
                        is_online,
                    }
                }).collect();
                drop(peers);
                let resp = RelayMessage::DeviceList { devices, target: Some(my_key.to_string()) };
                let _ = state.broadcast_tx.send(resp);
            }
            Err(e) => {
                let _ = state.broadcast_tx.send(RelayMessage::Private {
                    to: my_key.to_string(),
                    message: format!("Failed to load devices: {e}"),
                });
            }
        }
    }
}

pub async fn handle_device_label(
    state: &Arc<RelayState>,
    my_key: &str,
    public_key: String,
    label: String,
) {
    if let Ok(Some(name)) = state.db.name_for_key(my_key) {
        let keys = state.db.keys_for_name(&name).unwrap_or_default();
        if keys.contains(&public_key) {
            let label_trimmed = label.trim();
            if label_trimmed.len() > 32 {
                let _ = state.broadcast_tx.send(RelayMessage::Private {
                    to: my_key.to_string(),
                    message: "Label must be 32 characters or less.".to_string(),
                });
            } else {
                let _ = state.db.label_key(&name, &public_key, label_trimmed);
                if let Ok(keys) = state.db.keys_for_name_detailed(&name) {
                    let peers = state.peers.read().await;
                    let devices: Vec<DeviceInfo> = keys.into_iter().map(|(key, label, reg_at)| {
                        let is_online = peers.values().any(|p| p.public_key_hex == key);
                        DeviceInfo {
                            is_current: key == my_key,
                            public_key: key,
                            label,
                            registered_at: reg_at as u64,
                            is_online,
                        }
                    }).collect();
                    drop(peers);
                    let _ = state.broadcast_tx.send(RelayMessage::DeviceList { devices, target: Some(my_key.to_string()) });
                }
            }
        } else {
            let _ = state.broadcast_tx.send(RelayMessage::Private {
                to: my_key.to_string(),
                message: "That key doesn't belong to you.".to_string(),
            });
        }
    }
}

pub async fn handle_device_revoke(
    state: &Arc<RelayState>,
    my_key: &str,
    key_prefix: String,
) {
    if let Ok(Some(name)) = state.db.name_for_key(my_key) {
        if my_key.starts_with(&key_prefix) {
            let _ = state.broadcast_tx.send(RelayMessage::Private {
                to: my_key.to_string(),
                message: "Cannot revoke your current device. Use another device to revoke this one.".to_string(),
            });
        } else {
            match state.db.revoke_device(&name, &key_prefix) {
                Ok(revoked_keys) if !revoked_keys.is_empty() => {
                    let first = &revoked_keys[0];
                    let short: String = first.chars().take(16).collect();
                    let notice = format!("Device revoked: {}…", short);
                    let _ = state.broadcast_tx.send(RelayMessage::Private {
                        to: my_key.to_string(),
                        message: notice,
                    });
                    if let Ok(keys) = state.db.keys_for_name_detailed(&name) {
                        let peers = state.peers.read().await;
                        let devices: Vec<DeviceInfo> = keys.into_iter().map(|(key, label, reg_at)| {
                            let is_online = peers.values().any(|p| p.public_key_hex == key);
                            DeviceInfo {
                                is_current: key == my_key,
                                public_key: key,
                                label,
                                registered_at: reg_at as u64,
                                is_online,
                            }
                        }).collect();
                        drop(peers);
                        let _ = state.broadcast_tx.send(RelayMessage::DeviceList { devices, target: Some(my_key.to_string()) });
                    }
                }
                Ok(_) => {
                    let _ = state.broadcast_tx.send(RelayMessage::Private {
                        to: my_key.to_string(),
                        message: "No matching key found for your account.".to_string(),
                    });
                }
                Err(e) => {
                    let _ = state.broadcast_tx.send(RelayMessage::Private {
                        to: my_key.to_string(),
                        message: format!("Revoke failed: {e}"),
                    });
                }
            }
        }
    }
}

// ── Federation handlers ──

pub async fn handle_federation_hello(
    state: &Arc<RelayState>,
    my_key: &str,
    server_id: String,
    public_key: String,
    name: String,
    version: String,
    timestamp: u64,
    signature: String,
) {
    if let Ok(servers) = state.db.list_federated_servers() {
        if let Some(server) = servers.iter().find(|s| s.server_id == server_id || s.url == server_id) {
            if server.trust_tier >= 2 {
                let sig_valid = if let Some(ref stored_pk) = server.public_key {
                    verify_ed25519_signature(stored_pk, &timestamp.to_string(), timestamp, &signature)
                } else {
                    let _ = state.db.update_federated_server_info(&server_id, &name, Some(&public_key), false);
                    true
                };
                if sig_valid {
                    let fed_channels = state.db.get_federated_channels().unwrap_or_default();
                    let welcome = RelayMessage::FederationWelcome {
                        server_id: state.db.get_or_create_server_keypair().map(|(pk, _)| pk).unwrap_or_default(),
                        name: std::env::var("SERVER_NAME").unwrap_or_else(|_| "Humanity Relay".to_string()),
                        channels: fed_channels,
                    };
                    let _ = state.broadcast_tx.send(welcome);
                    let _ = state.db.update_federated_server_status(&server_id, "online");
                    tracing::info!("Federation: accepted hello from {} ({})", name, server_id);
                } else {
                    tracing::warn!("Federation: invalid signature from {}", server_id);
                }
            } else {
                tracing::warn!("Federation: rejected hello from {} — trust tier {} < 2", name, server.trust_tier);
            }
        }
    }
}

pub async fn handle_federated_chat(
    state: &Arc<RelayState>,
    server_id: String,
    server_name: String,
    from_name: String,
    content: String,
    timestamp: u64,
    channel: String,
    signature: Option<String>,
) {
    let accepted = if let Ok(servers) = state.db.list_federated_servers() {
        servers.iter().any(|s| (s.server_id == server_id || s.url == server_id) && s.trust_tier >= 2)
    } else { false };

    if accepted {
        if state.db.is_channel_federated(&channel).unwrap_or(false) {
            let federated_msg = RelayMessage::FederatedChat {
                server_id, server_name, from_name, content, timestamp, channel, signature,
            };
            let _ = state.broadcast_tx.send(federated_msg);
        }
    }
}

pub async fn handle_federation_welcome(
    state: &Arc<RelayState>,
    server_id: String,
    name: String,
    channels: Vec<String>,
) {
    tracing::info!("Federation: welcome from {} — federated channels: {:?}", name, channels);
    let _ = state.db.update_federated_server_status(&server_id, "online");
}

// ── Stream handlers ──

pub async fn handle_stream_start(
    state: &Arc<RelayState>,
    my_key: &str,
    title: String,
    category: String,
) {
    let role = state.db.get_role(my_key).unwrap_or_default();
    if role != "admin" {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "Only admins can stream to the relay.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
    } else {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let streamer_name = {
            let peers = state.peers.read().await;
            peers.get(my_key).and_then(|p| p.display_name.clone()).unwrap_or_else(|| "Unknown".to_string())
        };
        let db_id = state.db.create_stream(my_key, &title, &category).ok();
        let stream = ActiveStream {
            streamer_key: my_key.to_string(),
            streamer_name: streamer_name.clone(),
            title: title.clone(),
            category: category.clone(),
            started_at: now,
            viewer_keys: HashSet::new(),
            external_urls: Vec::new(),
            db_id,
        };
        *state.active_stream.write().await = Some(stream);
        tracing::info!("Stream started by {} ({}): {}", streamer_name, my_key, title);
        let info_msg = RelayMessage::StreamInfo {
            active: true,
            streamer_name: Some(streamer_name),
            streamer_key: Some(my_key.to_string()),
            title: Some(title),
            category: Some(category),
            viewer_count: 0,
            started_at: Some(now),
            external_urls: None,
        };
        let _ = state.broadcast_tx.send(info_msg);
    }
}

pub async fn handle_stream_stop(
    state: &Arc<RelayState>,
    my_key: &str,
) {
    let mut stream_lock = state.active_stream.write().await;
    if let Some(ref stream) = *stream_lock {
        let role = state.db.get_role(my_key).unwrap_or_default();
        if stream.streamer_key == my_key || role == "admin" {
            let viewer_peak = stream.viewer_keys.len() as i64;
            if let Some(db_id) = stream.db_id {
                let _ = state.db.end_stream(db_id, viewer_peak);
            }
            tracing::info!("Stream stopped by {}", my_key);
            *stream_lock = None;
            drop(stream_lock);
            let info_msg = RelayMessage::StreamInfo {
                active: false,
                streamer_name: None,
                streamer_key: None,
                title: None,
                category: None,
                viewer_count: 0,
                started_at: None,
                external_urls: None,
            };
            let _ = state.broadcast_tx.send(info_msg);
        }
    }
}

pub async fn handle_stream_offer(
    state: &Arc<RelayState>,
    my_key: &str,
    to: String,
    data: serde_json::Value,
) {
    tracing::info!("StreamOffer from {} to {}", &my_key[..8], &to[..8]);
    let _ = state.broadcast_tx.send(RelayMessage::StreamOffer {
        from: my_key.to_string(),
        to,
        data,
    });
}

pub async fn handle_stream_answer(
    state: &Arc<RelayState>,
    my_key: &str,
    to: String,
    data: serde_json::Value,
) {
    tracing::info!("StreamAnswer from {} to {}", &my_key[..8], &to[..8]);
    let _ = state.broadcast_tx.send(RelayMessage::StreamAnswer {
        from: my_key.to_string(),
        to,
        data,
    });
}

pub async fn handle_stream_ice(
    state: &Arc<RelayState>,
    my_key: &str,
    to: String,
    data: serde_json::Value,
) {
    tracing::info!("StreamIce from {} to {}", &my_key[..8], &to[..8]);
    let _ = state.broadcast_tx.send(RelayMessage::StreamIce {
        from: my_key.to_string(),
        to,
        data,
    });
}

pub async fn handle_stream_viewer_join(
    state: &Arc<RelayState>,
    my_key: &str,
) {
    tracing::info!("Stream viewer join from {}", my_key);
    let mut stream_lock = state.active_stream.write().await;
    if let Some(ref mut stream) = *stream_lock {
        stream.viewer_keys.insert(my_key.to_string());
        let count = stream.viewer_keys.len() as u32;
        let streamer_key = stream.streamer_key.clone();
        let info_msg = RelayMessage::StreamInfo {
            active: true,
            streamer_name: Some(stream.streamer_name.clone()),
            streamer_key: Some(stream.streamer_key.clone()),
            title: Some(stream.title.clone()),
            category: Some(stream.category.clone()),
            viewer_count: count,
            started_at: Some(stream.started_at),
            external_urls: Some(stream.external_urls.clone()),
        };
        drop(stream_lock);
        let _ = state.broadcast_tx.send(info_msg);
        tracing::info!("Sending __stream_viewer_ready__ to streamer {} for viewer {}", streamer_key, my_key);
        let notify = RelayMessage::Private {
            to: streamer_key,
            message: format!("__stream_viewer_ready__:{}", my_key),
        };
        let _ = state.broadcast_tx.send(notify);
    }
}

pub async fn handle_stream_viewer_leave(
    state: &Arc<RelayState>,
    my_key: &str,
) {
    let mut stream_lock = state.active_stream.write().await;
    if let Some(ref mut stream) = *stream_lock {
        stream.viewer_keys.remove(my_key);
        let count = stream.viewer_keys.len() as u32;
        let peak = stream.viewer_keys.len() as i64;
        if let Some(db_id) = stream.db_id {
            let _ = state.db.update_stream_viewer_peak(db_id, peak);
        }
        let info_msg = RelayMessage::StreamInfo {
            active: true,
            streamer_name: Some(stream.streamer_name.clone()),
            streamer_key: Some(stream.streamer_key.clone()),
            title: Some(stream.title.clone()),
            category: Some(stream.category.clone()),
            viewer_count: count,
            started_at: Some(stream.started_at),
            external_urls: Some(stream.external_urls.clone()),
        };
        drop(stream_lock);
        let _ = state.broadcast_tx.send(info_msg);
    }
}

pub async fn handle_stream_chat(
    state: &Arc<RelayState>,
    my_key: &str,
    content: String,
    source: String,
    source_user: Option<String>,
) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let from_name = {
        let peers = state.peers.read().await;
        peers.get(my_key).and_then(|p| p.display_name.clone())
    };
    {
        let stream = state.active_stream.read().await;
        if let Some(ref s) = *stream {
            if let Some(db_id) = s.db_id {
                let _ = state.db.store_stream_chat(
                    db_id, &content,
                    from_name.as_deref().unwrap_or("Unknown"),
                    &source,
                );
            }
        }
    }
    let chat_msg = RelayMessage::StreamChat {
        content,
        source,
        source_user,
        from: Some(my_key.to_string()),
        from_name,
        timestamp: now,
    };
    let _ = state.broadcast_tx.send(chat_msg);
}

pub async fn handle_stream_info_request(
    state: &Arc<RelayState>,
) {
    let stream = state.active_stream.read().await;
    let info_msg = if let Some(ref s) = *stream {
        RelayMessage::StreamInfo {
            active: true,
            streamer_name: Some(s.streamer_name.clone()),
            streamer_key: Some(s.streamer_key.clone()),
            title: Some(s.title.clone()),
            category: Some(s.category.clone()),
            viewer_count: s.viewer_keys.len() as u32,
            started_at: Some(s.started_at),
            external_urls: Some(s.external_urls.clone()),
        }
    } else {
        RelayMessage::StreamInfo {
            active: false,
            streamer_name: None,
            streamer_key: None,
            title: None,
            category: None,
            viewer_count: 0,
            started_at: None,
            external_urls: None,
        }
    };
    let _ = state.broadcast_tx.send(info_msg);
}

pub async fn handle_stream_set_external(
    state: &Arc<RelayState>,
    my_key: &str,
    urls: Vec<StreamExternalUrl>,
) {
    let role = state.db.get_role(my_key).unwrap_or_default();
    if role == "admin" {
        let mut stream_lock = state.active_stream.write().await;
        if let Some(ref mut stream) = *stream_lock {
            stream.external_urls = urls;
            let info_msg = RelayMessage::StreamInfo {
                active: true,
                streamer_name: Some(stream.streamer_name.clone()),
                streamer_key: Some(stream.streamer_key.clone()),
                title: Some(stream.title.clone()),
                category: Some(stream.category.clone()),
                viewer_count: stream.viewer_keys.len() as u32,
                started_at: Some(stream.started_at),
                external_urls: Some(stream.external_urls.clone()),
            };
            drop(stream_lock);
            let _ = state.broadcast_tx.send(info_msg);
        }
    }
}

// ── Project handlers ──

/// Build a ProjectData from a storage record + task count.
fn project_record_to_data(
    rec: &crate::storage::ProjectRecord,
    task_count: i64,
) -> ProjectData {
    ProjectData {
        id: rec.id.clone(),
        name: rec.name.clone(),
        description: rec.description.clone(),
        owner_key: rec.owner_key.clone(),
        visibility: rec.visibility.clone(),
        color: rec.color.clone(),
        icon: rec.icon.clone(),
        created_at: rec.created_at.clone(),
        task_count,
    }
}

pub async fn handle_project_list(
    state: &Arc<RelayState>,
    my_key: &str,
) {
    let projects_with_counts = state.db.get_projects_visible_to(my_key).unwrap_or_default();
    let projects: Vec<ProjectData> = projects_with_counts
        .iter()
        .map(|(rec, tc)| project_record_to_data(rec, *tc))
        .collect();
    let _ = state.broadcast_tx.send(RelayMessage::ProjectListResponse {
        target: Some(my_key.to_string()),
        projects,
    });
}

pub async fn handle_project_create(
    state: &Arc<RelayState>,
    my_key: &str,
    name: String,
    description: String,
    visibility: String,
    color: String,
    icon: String,
) {
    // Validate name.
    if name.trim().is_empty() || name.len() > 100 {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "Project name must be 1-100 characters.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
        return;
    }
    if description.len() > 2000 {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "Project description too long (max 2000 chars).".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
        return;
    }

    let valid_vis = ["public", "private", "members-only"];
    let vis = if valid_vis.contains(&visibility.as_str()) { &visibility } else { "public" };

    let id = uuid::Uuid::new_v4().to_string();
    match state.db.create_project(&id, &name, &description, my_key, vis, &color, &icon) {
        Ok(()) => {
            if let Ok(Some(rec)) = state.db.get_project_by_id(&id) {
                let pd = project_record_to_data(&rec, 0);
                let _ = state.broadcast_tx.send(RelayMessage::ProjectCreated { project: pd });
            }
        }
        Err(e) => {
            tracing::error!("Failed to create project: {e}");
        }
    }
}

pub async fn handle_project_update(
    state: &Arc<RelayState>,
    my_key: &str,
    id: String,
    name: String,
    description: String,
    visibility: String,
    color: String,
    icon: String,
) {
    if id == "default" {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "Cannot modify the default project.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
        return;
    }
    if name.trim().is_empty() || name.len() > 100 {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "Project name must be 1-100 characters.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
        return;
    }

    let valid_vis = ["public", "private", "members-only"];
    let vis = if valid_vis.contains(&visibility.as_str()) { &visibility } else { "public" };
    let is_admin = state.db.get_role(my_key).unwrap_or_default() == "admin";

    match state.db.update_project(&id, my_key, &name, &description, vis, &color, &icon, is_admin) {
        Ok(true) => {
            // Fetch updated record with task count.
            let projects = state.db.get_projects_visible_to(my_key).unwrap_or_default();
            if let Some((rec, tc)) = projects.iter().find(|(r, _)| r.id == id) {
                let pd = project_record_to_data(rec, *tc);
                let _ = state.broadcast_tx.send(RelayMessage::ProjectUpdated { project: pd });
            }
        }
        Ok(false) => {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: "Project not found or you don't have permission.".to_string(),
            };
            let _ = state.broadcast_tx.send(private);
        }
        Err(e) => tracing::error!("Project update error: {e}"),
    }
}

pub async fn handle_project_delete(
    state: &Arc<RelayState>,
    my_key: &str,
    id: String,
) {
    if id == "default" {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "Cannot delete the default project.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
        return;
    }
    let is_admin = state.db.get_role(my_key).unwrap_or_default() == "admin";
    match state.db.delete_project(&id, my_key, is_admin) {
        Ok(true) => {
            let _ = state.broadcast_tx.send(RelayMessage::ProjectDeleted { id });
        }
        Ok(false) => {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: "Project not found or you don't have permission.".to_string(),
            };
            let _ = state.broadcast_tx.send(private);
        }
        Err(e) => tracing::error!("Project delete error: {e}"),
    }
}
