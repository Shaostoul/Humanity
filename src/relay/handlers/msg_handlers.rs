//! Message handler functions extracted from handle_connection's match arms.
//! Each function handles one logical group of WebSocket message types.
//! Pure refactor — no behavior changes.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use rand::Rng;

use crate::relay::relay::*;
use crate::relay::storage::Storage;
use crate::relay::handlers::broadcast::*;
use crate::relay::handlers::utils::*;

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

// Full-PQ: the Ed25519 dual-sign key-rotation handler was removed.
// Keys derive deterministically from the BIP39 seed; "rotation" = restore
// from a new seed. The `key_rotations` table is left inert (the Inc6
// fresh-schema wipe simply won't carry it forward meaningfully).

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
                    bio: clean_bio.clone(),
                    socials: socials.clone(),
                    avatar_url: if avatar.is_empty() { None } else { Some(avatar.clone()) },
                    banner_url: if banner.is_empty() { None } else { Some(banner.clone()) },
                    pronouns: if pronoun.is_empty() { None } else { Some(pronoun.clone()) },
                    location: if loc.is_empty() { None } else { Some(loc.clone()) },
                    website: if w_site.is_empty() { None } else { Some(w_site.clone()) },
                    target: Some(my_key.to_string()),
                });

                // Also cache as a signed profile (key-based, for federation replication).
                // The signature is empty until clients sign profiles client-side; peers
                // accept empty-signature gossip under the trust-by-source model.
                // See `federation::should_accept_profile_gossip`.
                let ts = crate::relay::storage::now_millis();
                let _ = state.db.store_signed_profile(
                    my_key, name, &clean_bio, &avatar, &banner, &socials,
                    &pronoun, &loc, &w_site, ts, "",
                );

                // Gossip the profile update to federated servers. Forward the empty
                // signature today; once clients sign over `canonical_profile_message`,
                // pass the stored sig here and peers will verify it.
                crate::relay::handlers::federation::gossip_profile(
                    state, my_key, name, &clean_bio, &avatar, &banner,
                    &socials, &pronoun, &loc, &w_site, ts, "",
                ).await;
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
    // Fail-closed zero-knowledge guard (security review HIGH-1): the relay
    // refuses to store/forward a cleartext DM between human accounts. A
    // buggy or hostile client cannot downgrade a DM to plaintext — full-PQ
    // DMs are ALWAYS the sealed {v:1,r,s} envelope (`encrypted:true`).
    // `bot_` senders (AI agents) are exempt — they have no seal keypair
    // and their relay-mediated messages are not claimed to be E2EE. The
    // server-side `/dm` command builds RelayMessage::Dm directly and does
    // not pass through this handler, so it is unaffected.
    if !encrypted && !my_key.starts_with("bot_") {
        let _ = state.broadcast_tx.send(RelayMessage::Private {
            to: my_key.to_string(),
            message: "DM rejected: messages must be end-to-end encrypted. Hard-refresh (Ctrl+Shift+R) to update your client.".to_string(),
        });
        return;
    }
    let dm_role = state.db.get_role(my_key).unwrap_or_default();
    // Plaintext DMs are limited by visible characters. Encrypted DMs carry
    // an OPAQUE post-quantum ciphertext blob: ML-KEM-768 ek_ct alone is
    // ~1.45 KB of base64, and the client dual-seals (recipient + self, so
    // BOTH parties can read history on any device) — a 2 KB plaintext
    // becomes a ~9 KB envelope. Char-limiting ciphertext is meaningless;
    // the user-visible plaintext length is enforced client-side before
    // sealing. Allow a generous ceiling so PQ DMs aren't false-rejected.
    let dm_char_limit: usize = if encrypted {
        131_072 // 128 KB — far above a dual-sealed max-length plaintext DM
    } else if dm_role == "admin" {
        10_000
    } else {
        2_000
    };
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
                // Group voice channels use a synthetic "group:<id>" room_id (see the
                // group_list handler in src/lib.rs) -- there's no row for these in the
                // `channels` table, so `channel_voice_enabled` would always reject them.
                // Group channels are voice-capable by convention (the client always sets
                // voice_enabled: true for them); the real gate here is GROUP MEMBERSHIP,
                // not a channels-table lookup, so a non-member can't join by crafting or
                // guessing another group's id.
                let (allowed, deny_message, display_name_override) = if let Some(gid) = rid.strip_prefix("group:") {
                    match state.db.is_group_member(gid, my_key) {
                        Ok(true) => (true, None, state.db.get_user_groups(my_key).ok()
                            .and_then(|gs| gs.into_iter().find(|(id, ..)| id == gid))
                            .map(|(_, name, ..)| name)),
                        Ok(false) => (false, Some("You are not a member of this group.".to_string()), None),
                        Err(e) => {
                            tracing::error!("is_group_member check failed: {e}");
                            (false, Some("Could not verify group membership.".to_string()), None)
                        }
                    }
                } else {
                    // `rid` is the TEXT channel's string id. Voice is per-channel via
                    // the voice_enabled flag (v0.493), not the legacy voice_channels
                    // table — so validate + name from the channels table.
                    (state.db.channel_voice_enabled(&rid), Some("Voice is not enabled for this channel.".to_string()), None)
                };
                if !allowed {
                    let private = RelayMessage::Private { to: my_key.to_string(), message: deny_message.unwrap_or_default() };
                    let _ = state.broadcast_tx.send(private);
                } else {
                    let mut rooms = state.voice_rooms.write().await;
                    let room = rooms.entry(rid.clone()).or_insert_with(|| {
                        // `channel_display_name` has no row for a "group:<id>" synthetic id --
                        // use the group's real name (already resolved above) when this is a
                        // group room.
                        let name = display_name_override
                            .or_else(|| state.db.channel_display_name(&rid))
                            .unwrap_or_else(|| "Voice".to_string());
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

// ── Moderation handlers ──

/// Handle a moderation action (kick / ban / mute / mod / unmod) sent by
/// a moderator or admin from the user-profile modal.
///
/// Authorization:
///   - kick / mute      → requires moderator OR admin
///   - ban / mod / unmod → requires admin only
///
/// Effects per action:
///   - kick: DELETE FROM server_members for target; broadcast MemberLeft
///   - ban:  same as kick, PLUS persists to `banned_keys` (`Storage::ban_user`) --
///     the identify handshake (`src/relay/relay.rs`, `is_banned` check) rejects
///     any banned key and closes the socket, so they can never rejoin until an
///     admin unbans them. (This doc comment used to say the table was a TODO
///     and ban==kick; that was stale -- the real table + enforcement have
///     existed since before this comment was last touched, found + corrected
///     during the 2026-07-01 overnight chat-completeness sweep.)
///   - mute: persists to `muted_members` (`Storage::mute_user`/`is_muted`) --
///     a muted user can still read but message-send is rejected both in chat
///     (`src/relay/relay.rs`) and DMs (`handle_dm`, this file). Also stale in
///     the old version of this comment; corrected the same sweep.
///   - mod:  set target's role to "mod"
///   - unmod: set target's role to "member"
///
/// All actions report success / failure as a Private message to the caller.
pub async fn handle_mod_action(
    state: &Arc<RelayState>,
    my_key: &str,
    action: &str,
    target: &str,
    target_name: &str,
) {
    let my_role = state.db.get_role(my_key).unwrap_or_default();
    let is_admin = my_role == "admin";
    let is_mod = is_admin || my_role == "moderator" || my_role == "mod";
    if !is_mod {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "You don't have permission to perform moderation actions.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
        return;
    }
    // Admin-only actions.
    if matches!(action, "ban" | "mod" | "unmod" | "verify" | "unverify") && !is_admin {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: format!("'{action}' requires admin privileges."),
        };
        let _ = state.broadcast_tx.send(private);
        return;
    }
    // Caller must supply at least one identifier (key OR name).
    if target.is_empty() && target_name.is_empty() {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: format!("{action}: no target specified."),
        };
        let _ = state.broadcast_tx.send(private);
        return;
    }
    // Resolve EVERY public key this action could touch: the explicit
    // target key (if any) PLUS every key registered under target_name
    // (case-insensitive, multi-device). The protection checks below run
    // against this whole set so a name-only kick/ban can't sneak past
    // the role check just because no key was supplied. (v0.247 — closes
    // the documented name-only-bypass security gap.)
    let mut candidate_keys: Vec<String> = Vec::new();
    if !target.is_empty() {
        candidate_keys.push(target.to_string());
    }
    if !target_name.is_empty() {
        if let Ok(name_keys) = state.db.keys_for_name(target_name) {
            candidate_keys.extend(name_keys);
        }
    }
    candidate_keys.sort();
    candidate_keys.dedup();

    // Never let someone kick/ban/mute themselves through the moderation
    // UI — checked against the resolved key set so a name-only
    // self-target is caught too (previously only `target == my_key`).
    if matches!(action, "kick" | "ban" | "mute")
        && candidate_keys.iter().any(|k| k == my_key)
    {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: format!("You can't {action} yourself."),
        };
        let _ = state.broadcast_tx.send(private);
        return;
    }
    // Don't allow non-admins to act on a protected (admin/owner)
    // account. Now checks ALL resolved keys — if ANY registration under
    // the target name is an admin, a non-admin mod is refused, even on
    // the name-only path that used to skip this entirely.
    if !is_admin {
        let touches_protected = candidate_keys.iter().any(|k| {
            let r = state.db.get_role(k).unwrap_or_default();
            r == "admin" || r == "owner"
        });
        if touches_protected {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: "Only an admin can act on another admin.".to_string(),
            };
            let _ = state.broadcast_tx.send(private);
            return;
        }
    }

    let display_name = if !target.is_empty() {
        state.db.name_for_key(target).ok().flatten().unwrap_or_else(|| target[..8.min(target.len())].to_string())
    } else {
        target_name.to_string()
    };

    match action {
        "kick" | "ban" => {
            // Delete by public_key (preferred) AND/OR by name (fallback
            // for users with empty/unknown key — e.g. DesktopUser_4000
            // had an empty key in registered_names so key-based kick
            // was a no-op). Both paths run; whichever finds rows removes
            // them. Operator feedback 2026-05-12.
            let mut total_members_deleted: usize = 0;
            let mut total_names_deleted: usize = 0;
            // Every public key this action touches. Used to (a) force-close
            // any live socket for the user immediately (kick + ban) and
            // (b) persist a ban so they can't reconnect (ban only).
            let mut affected_keys: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            if !target.is_empty() {
                affected_keys.insert(target.to_string());
                if let Ok(true) = state.db.leave_server(target) {
                    total_members_deleted += 1;
                }
                total_names_deleted += state.db.delete_registered_name(target).unwrap_or(0);
            }
            if !target_name.is_empty() {
                // Name-based fallback — removes ALL rows matching this
                // display name (case-insensitive). For users with multiple
                // device keys this kicks all of them, which is the right
                // behaviour for a mod action.
                if let Ok(name_keys) = state.db.keys_for_name(target_name) {
                    for k in &name_keys {
                        affected_keys.insert(k.clone());
                        if let Ok(true) = state.db.leave_server(k) {
                            total_members_deleted += 1;
                        }
                    }
                }
                total_names_deleted += state.db
                    .delete_registered_names_by_name(target_name)
                    .unwrap_or(0);
            }

            let did_something = total_members_deleted > 0 || total_names_deleted > 0;

            // BAN: persist every affected key to the banned_keys table,
            // recording the display name (the kick path above already
            // deleted the registered_names rows, so the name has to be
            // captured here or the admin "Banned users" panel can't show
            // who it is). The identify handler (relay.rs connection gate)
            // rejects any banned key and closes the socket, so the user
            // can never rejoin until an admin unbans them. KICK is
            // transient — removed from the member list, may reconnect.
            if action == "ban" {
                for k in &affected_keys {
                    if k.is_empty() {
                        continue; // can't ban a keyless registration by key
                    }
                    if let Err(e) = state.db.ban_user(k, &display_name) {
                        tracing::error!("mod_action ban set_banned error: {e}");
                    }
                }
            }

            // Force-close any live socket for the affected keys (both kick
            // and ban). The per-connection send/recv tasks poll
            // kicked_keys and self-terminate; removing the peer stops new
            // broadcasts reaching them. kicked_keys is cleared on
            // disconnect (relay.rs) so a kicked user can reconnect; a
            // banned user is stopped by the persistent banned_keys gate.
            if !affected_keys.is_empty() {
                {
                    let mut kicked = state.kicked_keys.write().await;
                    for k in &affected_keys {
                        if !k.is_empty() {
                            kicked.insert(k.clone());
                        }
                    }
                }
                {
                    let mut peers = state.peers.write().await;
                    for k in &affected_keys {
                        peers.remove(k);
                    }
                }
            }

            if did_something {
                let _ = state.broadcast_tx.send(RelayMessage::MemberLeft {
                    public_key: target.to_string(),
                    reason: action.to_string(),
                });
                crate::relay::handlers::broadcast::broadcast_full_user_list(state).await;
                // Past-tense verb, spelled out so "ban" -> "banned"
                // (not "baned") and "kick" -> "kicked".
                let past = if action == "ban" { "Banned" } else { "Kicked" };
                let private = RelayMessage::Private {
                    to: my_key.to_string(),
                    message: format!("✓ {past} {display_name}."),
                };
                let _ = state.broadcast_tx.send(private);
            } else {
                let private = RelayMessage::Private {
                    to: my_key.to_string(),
                    message: format!("{display_name} wasn't a member of this server."),
                };
                let _ = state.broadcast_tx.send(private);
            }
        }
        "mute" | "unmute" => {
            // v0.246: mute is orthogonal to role (the old /mute clobbered
            // it). Resolve every key for this user (target + all keys
            // sharing the display name, for multi-device) and add/remove
            // each from muted_members. The user stays in the member list
            // — they just can't post until unmuted.
            let mut keys: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            if !target.is_empty() {
                keys.insert(target.to_string());
            }
            if !target_name.is_empty() {
                if let Ok(name_keys) = state.db.keys_for_name(target_name) {
                    for k in name_keys {
                        keys.insert(k);
                    }
                }
            }
            keys.retain(|k| !k.is_empty());
            if keys.is_empty() {
                let private = RelayMessage::Private {
                    to: my_key.to_string(),
                    message: format!(
                        "Can't {action} {display_name}: no resolvable account key \
                         (they may have an unregistered/empty key)."
                    ),
                };
                let _ = state.broadcast_tx.send(private);
                return;
            }
            let muting = action == "mute";
            for k in &keys {
                let res = if muting {
                    state.db.mute_user(k, &display_name)
                } else {
                    state.db.unmute_user(k)
                };
                if let Err(e) = res {
                    tracing::error!("mod_action {action} error: {e}");
                }
            }
            // Tell the actor + the affected user.
            let (actor_msg, target_msg) = if muting {
                (
                    format!("✓ Muted {display_name}. They can read but not post until unmuted."),
                    "You have been muted — you can read but not send messages.".to_string(),
                )
            } else {
                (
                    format!("✓ Unmuted {display_name}."),
                    "You have been unmuted — you can send messages again.".to_string(),
                )
            };
            let _ = state.broadcast_tx.send(RelayMessage::Private {
                to: my_key.to_string(),
                message: actor_msg,
            });
            for k in &keys {
                let _ = state.broadcast_tx.send(RelayMessage::Private {
                    to: k.clone(),
                    message: target_msg.clone(),
                });
            }
        }
        "mod" => {
            if let Err(e) = state.db.set_role(target, "mod") {
                tracing::error!("mod_action set_role mod error: {e}");
                return;
            }
            crate::relay::handlers::broadcast::broadcast_full_user_list(state).await;
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: format!("✓ Promoted {target_name} to moderator."),
            };
            let _ = state.broadcast_tx.send(private);
        }
        // Verified badge (v0.687; hardened v0.692 after the range review). The
        // single role column makes verify a role REPLACEMENT, so the arm must
        // never touch elevated targets: the unguarded version let one Verify
        // click silently demote a moderator, another admin, or the CALLER
        // THEMSELVES (a sole-admin lockout recoverable only by DB edit).
        "verify" | "unverify" => {
            // An empty key would upsert a bogus role row for "" and silently do
            // nothing for the real user (the DesktopUser_4000 empty-key case).
            if target.trim().is_empty() {
                let private = RelayMessage::Private {
                    to: my_key.to_string(),
                    message: format!("Can't {action} {target_name}: no registered key."),
                };
                let _ = state.broadcast_tx.send(private);
                return;
            }
            let current = state.db.get_role(target).unwrap_or_default();
            let ok_to_touch = if action == "verify" {
                // Only plain members (or role-less users) can be verified.
                matches!(current.as_str(), "" | "member" | "user" | "unverified")
            } else {
                // Only currently-verified users can be unverified.
                current == "verified"
            };
            if !ok_to_touch {
                let private = RelayMessage::Private {
                    to: my_key.to_string(),
                    message: format!(
                        "Can't {action} {target_name}: their role is '{current}'. \
                         Use the role controls for mods/admins.",
                        current = if current.is_empty() { "member" } else { &current }
                    ),
                };
                let _ = state.broadcast_tx.send(private);
                return;
            }
            let new_role = if action == "verify" { "verified" } else { "member" };
            if let Err(e) = state.db.set_role(target, new_role) {
                tracing::error!("mod_action set_role {new_role} error: {e}");
                return;
            }
            // Push the fresh role to every client NOW -- without this the V
            // badge only appeared after a reconnect (range-review follow-up).
            crate::relay::handlers::broadcast::broadcast_full_user_list(state).await;
            let verbed = if action == "verify" { "Verified" } else { "Unverified" };
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: format!("✓ {verbed} {target_name}."),
            };
            let _ = state.broadcast_tx.send(private);
        }
        "unmod" => {
            if let Err(e) = state.db.set_role(target, "member") {
                tracing::error!("mod_action set_role member error: {e}");
                return;
            }
            crate::relay::handlers::broadcast::broadcast_full_user_list(state).await;
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: format!("✓ Demoted {target_name} to member."),
            };
            let _ = state.broadcast_tx.send(private);
        }
        _ => {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: format!("Unknown moderation action '{action}'."),
            };
            let _ = state.broadcast_tx.send(private);
        }
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

// ── Notification Preferences handlers ──

pub async fn handle_update_notification_prefs(
    state: &Arc<RelayState>,
    my_key: &str,
    dm: bool,
    mentions: bool,
    tasks: bool,
    dnd_start: Option<String>,
    dnd_end: Option<String>,
) {
    if let Err(e) = state.db.save_notification_prefs(
        my_key,
        dm,
        mentions,
        tasks,
        dnd_start.as_deref(),
        dnd_end.as_deref(),
    ) {
        tracing::error!("Failed to save notification prefs: {e}");
        return;
    }
    // Send back confirmation with the saved prefs.
    let _ = state.broadcast_tx.send(RelayMessage::NotificationPrefsData {
        dm,
        mentions,
        tasks,
        dnd_start,
        dnd_end,
        target: Some(my_key.to_string()),
    });
}

pub async fn handle_get_notification_prefs(
    state: &Arc<RelayState>,
    my_key: &str,
) {
    let prefs = state.db.get_notification_prefs(my_key);
    let (dm, mentions, tasks, dnd_start, dnd_end) = match prefs {
        Some(p) => (p.dm_enabled, p.mentions_enabled, p.tasks_enabled, p.dnd_start, p.dnd_end),
        None => (true, true, true, None, None),
    };
    let _ = state.broadcast_tx.send(RelayMessage::NotificationPrefsData {
        dm,
        mentions,
        tasks,
        dnd_start,
        dnd_end,
        target: Some(my_key.to_string()),
    });
}

// ── Listing Message handlers (buyer-seller conversations) ──

pub async fn handle_listing_message_send(
    state: &Arc<RelayState>,
    my_key: &str,
    listing_id: String,
    content: String,
) {
    // Validate content.
    let content = content.trim().to_string();
    if content.is_empty() || content.len() > 2000 {
        let _ = state.broadcast_tx.send(RelayMessage::Private {
            to: my_key.to_string(),
            message: "Message must be 1-2000 characters.".to_string(),
        });
        return;
    }

    // Verify listing exists.
    if state.db.get_listing_by_id(&listing_id).ok().flatten().is_none() {
        let _ = state.broadcast_tx.send(RelayMessage::Private {
            to: my_key.to_string(),
            message: "Listing not found.".to_string(),
        });
        return;
    }

    let sender_name = state.db.name_for_key(my_key).ok().flatten().unwrap_or_else(|| "Anonymous".to_string());
    let timestamp = crate::relay::storage::now_millis() as i64;

    match state.db.create_listing_message(&listing_id, my_key, Some(&sender_name), &content, timestamp) {
        Ok(id) => {
            let msg_data = ListingMessageData {
                id,
                listing_id: listing_id.clone(),
                sender_key: my_key.to_string(),
                sender_name: Some(sender_name),
                content,
                timestamp,
            };
            let _ = state.broadcast_tx.send(RelayMessage::ListingMessageNew {
                listing_id,
                message: msg_data,
            });
        }
        Err(e) => {
            tracing::error!("Failed to create listing message: {e}");
        }
    }
}

pub async fn handle_listing_message_history(
    state: &Arc<RelayState>,
    my_key: &str,
    listing_id: String,
) {
    let records = state.db.get_listing_messages(&listing_id, 100);
    let messages: Vec<ListingMessageData> = records.iter().map(listing_message_from_db).collect();
    let _ = state.broadcast_tx.send(RelayMessage::ListingMessages {
        listing_id,
        messages,
        target: Some(my_key.to_string()),
    });
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
    // Reject replays before doing any DB work. (Security audit 2026-05-03 H2.)
    if !crate::relay::handlers::broadcast::is_timestamp_fresh(timestamp) {
        tracing::warn!(
            "Federation: rejected hello from {} — timestamp outside ±5 min window (replay?)",
            server_id
        );
        return;
    }

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
    // Security audit 2026-05-03 H3: previously this handler only checked
    // that the source server was trust-tier ≥ 2 and that the channel was
    // federated. The signature field was passed through to broadcast
    // without ever being verified — a compromised tier-2 server could
    // forge chat from any name. Now: reject stale timestamps, then
    // require + verify the signature against the source server's stored
    // pubkey. Unsigned messages are rejected outright.
    if !crate::relay::handlers::broadcast::is_timestamp_fresh(timestamp) {
        tracing::warn!(
            "Federation: rejected chat from {} — timestamp outside ±5 min window (replay?)",
            server_id
        );
        return;
    }

    let server = match state.db.list_federated_servers() {
        Ok(list) => list.into_iter().find(|s| s.server_id == server_id || s.url == server_id),
        Err(_) => None,
    };
    let Some(server) = server else {
        tracing::warn!("Federation: rejected chat from unknown server {}", server_id);
        return;
    };
    if server.trust_tier < 2 {
        tracing::warn!("Federation: rejected chat from {} — trust tier {} < 2", server_id, server.trust_tier);
        return;
    }

    // Verify signature. Canonical message: "fed_chat\n<from_name>\n<channel>\n<content>"
    // The trailing `timestamp` arg goes into the `\n<timestamp>` suffix that
    // verify_ed25519_signature appends; this is the same shape as profile gossip.
    let canonical = format!("fed_chat\n{}\n{}\n{}", from_name, channel, content);
    let sig_valid = match (signature.as_deref(), server.public_key.as_deref()) {
        (Some(sig), Some(pk)) => verify_ed25519_signature(pk, &canonical, timestamp, sig),
        _ => false,
    };
    if !sig_valid {
        tracing::warn!(
            "Federation: rejected chat from {} on {} — invalid or missing signature",
            server_id, channel
        );
        return;
    }

    if !state.db.is_channel_federated(&channel).unwrap_or(false) {
        return;
    }
    let federated_msg = RelayMessage::FederatedChat {
        server_id, server_name, from_name, content, timestamp, channel, signature,
    };
    let _ = state.broadcast_tx.send(federated_msg);
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
    // Capability composition (v0.239, docs/design/roles-system.md):
    //   effective_can_stream = server master switch AND role.can_stream
    // The server-wide video_streaming_enabled is the master kill-switch;
    // the per-role can_stream decides who, when it's on. Seed defaults:
    // mod + admin can_stream=1, so the operator opts other roles in by
    // editing/creating a role (e.g. a "Family" role with can_stream=1).
    let role = state.db.get_role(my_key).unwrap_or_default();
    let settings = state.db.get_server_settings().unwrap_or_default();
    let rd = state.db.role_def(&role);
    let may_stream = settings.video_streaming_enabled && rd.can_stream;
    if !may_stream {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: if !settings.video_streaming_enabled {
                "Streaming is disabled server-wide (enable it in Server Settings).".to_string()
            } else {
                format!("Your role \"{}\" is not permitted to stream on this server.", rd.label)
            },
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
            peak_viewers: 0,
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
            // The tracked high-water mark, not the live count at stop time --
            // by the time a stream ends most viewers have often already left,
            // so `viewer_keys.len()` here is frequently 0 or far below the
            // real peak (see ActiveStream::peak_viewers' doc comment).
            let viewer_peak = stream.peak_viewers as i64;
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
        let count = stream.viewer_keys.len();
        // The count is highest right here, at a join -- this is the ONLY
        // place the true peak can be observed, so record it now rather than
        // waiting to read a (by-then-lower) count at leave/stop time.
        stream.peak_viewers = stream.peak_viewers.max(count);
        let count = count as u32;
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
        // Persist the tracked high-water mark, NOT the post-leave live count
        // (which just decreased and is never the actual peak -- see
        // ActiveStream::peak_viewers' doc comment).
        let peak = stream.peak_viewers as i64;
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
    rec: &crate::relay::storage::ProjectRecord,
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

// ── Trade handlers ──

/// Helper: convert a TradeRecord to a TradeDataPayload for sending to clients.
fn trade_to_payload(t: &crate::relay::storage::TradeRecord) -> TradeDataPayload {
    let initiator_items: Vec<TradeItem> = serde_json::from_str(&t.initiator_items).unwrap_or_default();
    let recipient_items: Vec<TradeItem> = serde_json::from_str(&t.recipient_items).unwrap_or_default();
    TradeDataPayload {
        id: t.id.clone(),
        initiator_key: t.initiator_key.clone(),
        recipient_key: t.recipient_key.clone(),
        status: t.status.clone(),
        initiator_items,
        recipient_items,
        initiator_confirmed: t.initiator_confirmed,
        recipient_confirmed: t.recipient_confirmed,
        created_at: t.created_at,
        completed_at: t.completed_at,
        message: t.message.clone(),
    }
}

/// Send trade data to both parties. TARGETED private wrappers only (v0.756):
/// an earlier untargeted `TradeData` broadcast here delivered every trade's
/// item lists to every connected client; the private `__trade_data__:` path
/// (which web and native both consume) was always the intended delivery.
fn send_trade_data(state: &Arc<RelayState>, trade: &crate::relay::storage::TradeRecord) {
    let payload = trade_to_payload(trade);
    let json = serde_json::to_string(&RelayMessage::TradeData { trade: payload }).unwrap_or_default();
    let priv_init = RelayMessage::Private {
        to: trade.initiator_key.clone(),
        message: format!("__trade_data__:{}", json),
    };
    let priv_recv = RelayMessage::Private {
        to: trade.recipient_key.clone(),
        message: format!("__trade_data__:{}", json),
    };
    let _ = state.broadcast_tx.send(priv_init);
    let _ = state.broadcast_tx.send(priv_recv);
}

pub async fn handle_trade_request(
    state: &Arc<RelayState>,
    my_key: &str,
    raw: &serde_json::Value,
) {
    let target_key = match raw.get("target_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() && k != my_key => k,
        _ => {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: "Invalid trade target.".to_string(),
            };
            let _ = state.broadcast_tx.send(private);
            return;
        }
    };
    let message = raw.get("message").and_then(|v| v.as_str()).unwrap_or("");

    // Check target is online.
    let target_online = state.peers.read().await.contains_key(target_key);
    if !target_online {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "Trade target is not online.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
        return;
    }

    // Limit active trades per user (max 10).
    if let Ok(trades) = state.db.get_trades_for_user(my_key) {
        let active = trades.iter().filter(|t| t.status == "pending" || t.status == "active").count();
        if active >= 10 {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: "Too many active trades (max 10). Cancel some first.".to_string(),
            };
            let _ = state.broadcast_tx.send(private);
            return;
        }
    }

    let trade_id = format!("trade_{}", rand::rng().random::<u64>());
    match state.db.create_trade(&trade_id, my_key, target_key, if message.is_empty() { None } else { Some(message) }) {
        Ok(()) => {
            if let Ok(Some(trade)) = state.db.get_trade(&trade_id) {
                send_trade_data(state, &trade);
            }
        }
        Err(e) => tracing::error!("Trade create error: {e}"),
    }
}

pub async fn handle_trade_response(
    state: &Arc<RelayState>,
    my_key: &str,
    raw: &serde_json::Value,
) {
    let trade_id = match raw.get("trade_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return,
    };
    let accepted = raw.get("accepted").and_then(|v| v.as_bool()).unwrap_or(false);

    // Only the recipient can respond.
    if let Ok(Some(trade)) = state.db.get_trade(trade_id) {
        if trade.recipient_key != my_key {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: "Only the trade recipient can accept/reject.".to_string(),
            };
            let _ = state.broadcast_tx.send(private);
            return;
        }
        match state.db.respond_to_trade(trade_id, accepted) {
            Ok(true) => {
                if let Ok(Some(updated)) = state.db.get_trade(trade_id) {
                    send_trade_data(state, &updated);
                }
            }
            Ok(false) => {
                let private = RelayMessage::Private {
                    to: my_key.to_string(),
                    message: "Trade not found or not pending.".to_string(),
                };
                let _ = state.broadcast_tx.send(private);
            }
            Err(e) => tracing::error!("Trade response error: {e}"),
        }
    }
}

pub async fn handle_trade_update_items(
    state: &Arc<RelayState>,
    my_key: &str,
    raw: &serde_json::Value,
) {
    let trade_id = match raw.get("trade_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return,
    };
    let items = match raw.get("items") {
        Some(v) => v.to_string(),
        None => return,
    };

    // Validate items JSON parses as an array.
    if serde_json::from_str::<Vec<TradeItem>>(&items).is_err() {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: "Invalid items format.".to_string(),
        };
        let _ = state.broadcast_tx.send(private);
        return;
    }

    if let Ok(Some(trade)) = state.db.get_trade(trade_id) {
        let side = if my_key == trade.initiator_key {
            "initiator"
        } else if my_key == trade.recipient_key {
            "recipient"
        } else {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: "You are not part of this trade.".to_string(),
            };
            let _ = state.broadcast_tx.send(private);
            return;
        };

        match state.db.update_trade_items(trade_id, side, &items) {
            Ok(true) => {
                if let Ok(Some(updated)) = state.db.get_trade(trade_id) {
                    send_trade_data(state, &updated);
                }
            }
            Ok(false) => {
                let private = RelayMessage::Private {
                    to: my_key.to_string(),
                    message: "Trade not active or not found.".to_string(),
                };
                let _ = state.broadcast_tx.send(private);
            }
            Err(e) => tracing::error!("Trade update items error: {e}"),
        }
    }
}

pub async fn handle_trade_confirm(
    state: &Arc<RelayState>,
    my_key: &str,
    raw: &serde_json::Value,
) {
    let trade_id = match raw.get("trade_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return,
    };

    match state.db.confirm_trade(trade_id, my_key) {
        Ok((true, both_confirmed)) => {
            if both_confirmed {
                // Both confirmed — complete the trade.
                match state.db.complete_trade(trade_id) {
                    Ok(true) => {
                        if let Ok(Some(completed)) = state.db.get_trade(trade_id) {
                            send_trade_data(state, &completed);
                            // Also send a completion notification.
                            let comp_msg1 = RelayMessage::Private {
                                to: completed.initiator_key.clone(),
                                message: format!("__trade_complete__:{{\"trade_id\":\"{}\"}}", trade_id),
                            };
                            let comp_msg2 = RelayMessage::Private {
                                to: completed.recipient_key.clone(),
                                message: format!("__trade_complete__:{{\"trade_id\":\"{}\"}}", trade_id),
                            };
                            let _ = state.broadcast_tx.send(comp_msg1);
                            let _ = state.broadcast_tx.send(comp_msg2);
                        }
                    }
                    Ok(false) => tracing::warn!("Trade completion failed for {trade_id}"),
                    Err(e) => tracing::error!("Trade complete error: {e}"),
                }
            } else {
                // Only one side confirmed so far — notify both.
                if let Ok(Some(updated)) = state.db.get_trade(trade_id) {
                    send_trade_data(state, &updated);
                }
            }
        }
        Ok((false, _)) => {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: "Trade not active, not found, or you're not part of it.".to_string(),
            };
            let _ = state.broadcast_tx.send(private);
        }
        Err(e) => tracing::error!("Trade confirm error: {e}"),
    }
}

pub async fn handle_trade_cancel(
    state: &Arc<RelayState>,
    my_key: &str,
    raw: &serde_json::Value,
) {
    let trade_id = match raw.get("trade_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return,
    };

    match state.db.cancel_trade(trade_id, my_key) {
        Ok(true) => {
            if let Ok(Some(cancelled)) = state.db.get_trade(trade_id) {
                send_trade_data(state, &cancelled);
            }
        }
        Ok(false) => {
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: "Trade not found, already completed, or you're not part of it.".to_string(),
            };
            let _ = state.broadcast_tx.send(private);
        }
        Err(e) => tracing::error!("Trade cancel error: {e}"),
    }
}

pub async fn handle_trade_list_request(
    state: &Arc<RelayState>,
    my_key: &str,
) {
    match state.db.get_trades_for_user(my_key) {
        Ok(trades) => {
            let payloads: Vec<TradeDataPayload> = trades.iter().map(trade_to_payload).collect();
            let json = serde_json::json!({
                "type": "trade_list",
                "trades": payloads,
            }).to_string();
            let private = RelayMessage::Private {
                to: my_key.to_string(),
                message: format!("__trade_list__:{}", json),
            };
            let _ = state.broadcast_tx.send(private);
        }
        Err(e) => tracing::error!("Trade list error: {e}"),
    }
}

// ── Game state handlers ──

/// Handle a client joining the game world.
/// Creates a player entity in GameWorld, sends Welcome with world snapshot,
/// and broadcasts PlayerJoined to all other clients.
pub async fn handle_game_join(
    state: &Arc<RelayState>,
    my_key: &str,
    raw: &serde_json::Value,
) {
    let player_name = raw.get("player_name")
        .and_then(|v| v.as_str())
        .unwrap_or("Anonymous")
        .to_string();

    // ── Game-world ban gate (v0.474) ──
    // Playing on the shared world is a PRIVILEGE; a game ban blocks the spawn
    // here, BEFORE the world lock, so a banned player never gets an entity and
    // never broadcasts a join. This reads ONLY game_banned_keys and never the
    // chat banned_keys table -- a game-banned user keeps full chat + DM access
    // (free speech is a right). Bots (key starts with "bot_") are exempt, same
    // carve-out as the identify gate. FAIL CLOSED: a DB error denies the join
    // (this is a moderation gate, unlike the fail-open chat connect check).
    if !my_key.starts_with("bot_") {
        match state.db.is_game_banned(my_key) {
            Ok(Some(ban)) => {
                send_game_private(state, my_key, &serde_json::json!({
                    "type": "game_join_denied",
                    "reason": ban.reason,
                    "chat_unaffected": true,
                    "message": "You are banned from the game world. Chat is unaffected.",
                })).await;
                tracing::info!("Game-join denied for banned key {} (chat unaffected)", my_key);
                return;
            }
            Ok(None) => {}
            Err(e) => {
                tracing::error!("is_game_banned failed for {}: {} -- denying join (fail closed)", my_key, e);
                send_game_private(state, my_key, &serde_json::json!({
                    "type": "game_join_denied",
                    "reason": "server error",
                    "chat_unaffected": true,
                    "message": "Could not verify game access right now. Chat is unaffected; try again shortly.",
                })).await;
                return;
            }
        }
    }

    let mut world = state.game_world.write().await;

    // Don't create duplicate player entities.
    if world.find_player_entity(my_key).is_some() {
        tracing::warn!("Player {} already in game world, ignoring duplicate join", my_key);
        return;
    }

    // Spawn player at default position. spawn_player always grants the
    // explore_ship starter quest with zeroed stats.
    let spawn_pos = [0.0_f32, 1.0, 0.0];
    let player_id = world.spawn_player(my_key, spawn_pos);

    // Re-seed a RETURNING player from durable storage so they keep their
    // quest progress / XP / reputation across relay restarts (or across a
    // world-snapshot version bump, which is keyed separately). A brand-new
    // player has no saved row and keeps the fresh explore_ship grant above.
    match state.db.load_player_progress(my_key) {
        Ok(Some(p)) => {
            world.seed_player_progress(
                player_id,
                p.current_quest.as_deref(),
                &p.completed_quests,
                p.xp,
                p.reputation,
            );
            tracing::info!(
                "Restored player progress for {}: quest={:?}, xp={}, rep={}",
                my_key, p.current_quest, p.xp, p.reputation
            );
        }
        Ok(None) => {} // new player — nothing to restore
        Err(e) => tracing::warn!("Could not load player progress for {}: {e}", my_key),
    }

    // Stamp the display name onto the player entity (v0.774) so the world
    // snapshot a LATER joiner receives carries real names for people already
    // present -- without this they'd all read "Player" until they moved. The
    // game_player_joined broadcast already carries the name for the join-after
    // case; this closes the join-before (snapshot) case. Additive component,
    // backward-compatible with existing persisted snapshots.
    if let Some(e) = world.entities.get_mut(&player_id) {
        if let Some(obj) = e.components.as_object_mut() {
            obj.insert("name".to_string(), serde_json::json!(player_name));
        }
    }

    // Build world snapshot for the joiner.
    let snapshot = world.snapshot();
    let game_time = world.game_time;
    // Surface the starter quest in the welcome payload so AI agents
    // (and humans) know what to do without parsing world_snapshot.
    let current_quest = world
        .entities
        .get(&player_id)
        .and_then(|e| e.components.get("current_quest"))
        .cloned();
    // Surface the room list so AI agents can navigate the explore_ship
    // quest without a separate query — each entry has id, name, type,
    // position, size, and the player-spawnable center point.
    let rooms_summary: Vec<serde_json::Value> = world.rooms.iter().map(|r| {
        let center = [
            r.position[0] + r.size[0] / 2.0,
            r.position[1] + 1.0,
            r.position[2] + r.size[2] / 2.0,
        ];
        serde_json::json!({
            "id": r.id,
            "name": r.name,
            "room_type": r.room_type,
            "position": r.position,
            "size": r.size,
            "center": center,
        })
    }).collect();
    drop(world);

    // Build snapshot JSON values.
    let snapshot_json: Vec<serde_json::Value> = snapshot.iter().map(|s| {
        serde_json::json!({
            "entity_id": s.entity_id,
            "entity_type": s.entity_type,
            "position": s.position,
            "rotation": s.rotation,
            "components": s.components,
        })
    }).collect();

    // Send Welcome to the joining player.
    let mut welcome = serde_json::json!({
        "type": "game_welcome",
        "player_id": player_id,
        "world_snapshot": snapshot_json,
        "game_time": game_time,
        "rooms": rooms_summary,
    });
    if let Some(q) = current_quest {
        welcome["current_quest"] = q;
    }
    let private = RelayMessage::Private {
        to: my_key.to_string(),
        message: format!("__game__:{}", welcome),
    };
    let _ = state.broadcast_tx.send(private);

    // Broadcast PlayerJoined to all clients.
    let joined = serde_json::json!({
        "type": "game_player_joined",
        "player_id": player_id,
        "name": player_name,
        "position": spawn_pos,
    });
    let _ = state.broadcast_tx.send(RelayMessage::System {
        message: format!("__game__:{}", joined),
    });

    tracing::info!("Game: player '{}' ({}) joined as entity {}", player_name, my_key, player_id);
}

// ── Game admin: game-world bans (v0.474), SEPARATE from chat moderation ──
//
// These three handlers mirror the chat ban handlers (relay.rs BannedListRequest
// / Unban) but operate ONLY on the game_banned_keys table. They reuse the exact
// same authoritative gate -- `state.db.get_role(my_key)` must be "admin" or
// "owner" -- so there is no new auth surface. The socket's `my_key` is already
// proven by the two-phase Dilithium identify challenge. Replies go out via
// `send_game_private` (a Private targeted to the requesting admin), so the ban
// list never leaks to non-admins and no broadcast-loop edit is needed.

/// True if this key is an admin or owner (the game-admin gate). Defaults to
/// false (deny) on any DB error -- consistent with refusing the action.
fn is_game_admin(state: &Arc<RelayState>, my_key: &str) -> bool {
    let r = state.db.get_role(my_key).unwrap_or_default();
    r == "admin" || r == "owner"
}

/// Admin issues a game-world ban. Refuses non-admins, self-targets, and
/// protected (admin/owner) targets. Records `banned_by = my_key` for audit,
/// evicts the target from the live WORLD only (chat untouched), then pushes the
/// refreshed ban list back to the requesting admin.
pub async fn handle_game_ban(state: &Arc<RelayState>, my_key: &str, raw: &serde_json::Value) {
    if !is_game_admin(state, my_key) {
        send_game_private(state, my_key, &serde_json::json!({
            "type": "game_admin_error",
            "message": "Not authorized: game bans require an admin or owner role.",
        })).await;
        return;
    }
    let target = raw.get("target").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    let reason = raw.get("reason").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if target.is_empty() {
        send_game_private(state, my_key, &serde_json::json!({
            "type": "game_admin_error",
            "message": "No target public key provided.",
        })).await;
        return;
    }
    if target == my_key {
        send_game_private(state, my_key, &serde_json::json!({
            "type": "game_admin_error",
            "message": "You can't game-ban yourself.",
        })).await;
        return;
    }
    // Don't let an admin game-ban another admin/owner.
    let target_role = state.db.get_role(&target).unwrap_or_default();
    if target_role == "admin" || target_role == "owner" {
        send_game_private(state, my_key, &serde_json::json!({
            "type": "game_admin_error",
            "message": "That account is protected (admin/owner) and can't be game-banned.",
        })).await;
        return;
    }
    // v1: account-wide game ban (character_id = None).
    if let Err(e) = state.db.game_ban(&target, None, &reason, my_key) {
        tracing::error!("game_ban({}) failed: {}", target, e);
        send_game_private(state, my_key, &serde_json::json!({
            "type": "game_admin_error",
            "message": "Failed to record the game ban.",
        })).await;
        return;
    }
    tracing::info!("Game-ban issued by {} against {} (reason: {})", my_key, target, reason);
    // Evict from the live world only -- this despawns + broadcasts
    // game_player_left; it must NOT close the chat socket. handle_game_disconnect
    // is exactly that world-scoped eviction.
    handle_game_disconnect(state, &target).await;
    // Push the refreshed list back to the issuing admin.
    handle_game_banned_list(state, my_key).await;
}

/// Admin lifts a game-world ban (account-wide). Refuses non-admins.
pub async fn handle_game_unban(state: &Arc<RelayState>, my_key: &str, raw: &serde_json::Value) {
    if !is_game_admin(state, my_key) {
        send_game_private(state, my_key, &serde_json::json!({
            "type": "game_admin_error",
            "message": "Not authorized: game unbans require an admin or owner role.",
        })).await;
        return;
    }
    let target = raw.get("target").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if target.is_empty() {
        return;
    }
    if let Err(e) = state.db.game_unban(&target, None) {
        tracing::error!("game_unban({}) failed: {}", target, e);
    } else {
        tracing::info!("Game-unban issued by {} for {}", my_key, target);
    }
    handle_game_banned_list(state, my_key).await;
}

/// Admin requests the current game-ban list. Refuses non-admins. The reply is
/// targeted privately to the requesting admin (never broadcast).
pub async fn handle_game_banned_list(state: &Arc<RelayState>, my_key: &str) {
    if !is_game_admin(state, my_key) {
        send_game_private(state, my_key, &serde_json::json!({
            "type": "game_admin_error",
            "message": "Not authorized: the game-ban list is admin/owner only.",
        })).await;
        return;
    }
    let bans = state.db.list_game_bans().unwrap_or_default();
    send_game_private(state, my_key, &serde_json::json!({
        "type": "game_banned_list",
        "users": bans,
    })).await;
}

/// Handle a position update from a game client.
/// Validates the update, applies it server-side, and relays to other clients.
pub async fn handle_game_position_update(
    state: &Arc<RelayState>,
    my_key: &str,
    raw: &serde_json::Value,
) {
    let position: [f32; 3] = match raw.get("position").and_then(|v| {
        let arr = v.as_array()?;
        if arr.len() != 3 { return None; }
        Some([
            arr[0].as_f64()? as f32,
            arr[1].as_f64()? as f32,
            arr[2].as_f64()? as f32,
        ])
    }) {
        Some(p) => p,
        None => return,
    };

    let rotation: [f32; 4] = raw.get("rotation").and_then(|v| {
        let arr = v.as_array()?;
        if arr.len() != 4 { return None; }
        Some([
            arr[0].as_f64()? as f32,
            arr[1].as_f64()? as f32,
            arr[2].as_f64()? as f32,
            arr[3].as_f64()? as f32,
        ])
    }).unwrap_or([0.0, 0.0, 0.0, 1.0]);

    let velocity: [f32; 3] = raw.get("velocity").and_then(|v| {
        let arr = v.as_array()?;
        if arr.len() != 3 { return None; }
        Some([
            arr[0].as_f64()? as f32,
            arr[1].as_f64()? as f32,
            arr[2].as_f64()? as f32,
        ])
    }).unwrap_or([0.0, 0.0, 0.0]);

    let timestamp = raw.get("timestamp")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let mut world = state.game_world.write().await;
    let player_id = match world.find_player_entity(my_key) {
        Some(id) => id,
        None => return, // Not in the game world
    };

    // Server-side validation: reject teleportation (> 100 units per update).
    if let Some(entity) = world.entities.get(&player_id) {
        let dx = position[0] - entity.position[0];
        let dy = position[1] - entity.position[1];
        let dz = position[2] - entity.position[2];
        let dist_sq = dx * dx + dy * dy + dz * dz;
        if dist_sq > 100.0 * 100.0 {
            tracing::warn!("Game: rejected teleport from {} (dist={})", my_key, dist_sq.sqrt());
            return;
        }
    }

    world.update_position(player_id, position, rotation);

    // Quest progress: record room entry on the player's explore_ship quest.
    // record_room_visit returns Some(progress) only when the room is new for
    // this player, so we only fire the broadcast when something actually changed.
    // Also pick a greeting from the resident NPC for a flavorful first entry.
    let (quest_progress, greeting) = if let Some(room) = world.room_for_position(position) {
        let progress = world.record_room_visit(player_id, &room.id);
        let greeting = if progress.is_some() {
            world.pick_room_greeting(&room.id)
        } else {
            None
        };
        (progress, greeting)
    } else {
        (None, None)
    };

    drop(world);

    // Relay to all other game clients via broadcast.
    let update = serde_json::json!({
        "type": "game_position_update",
        "player_id": player_id,
        "position": position,
        "rotation": rotation,
        "velocity": velocity,
        "timestamp": timestamp,
    });
    let _ = state.broadcast_tx.send(RelayMessage::System {
        message: format!("__game__:{}", update),
    });

    // If the position update advanced a quest, send a private update to the
    // player and broadcast a public completion event when they finish.
    if let Some(progress) = quest_progress {
        let event_type = if progress.complete {
            "game_quest_completed"
        } else {
            "game_quest_progress"
        };
        let payload = serde_json::json!({
            "type": event_type,
            "player_id": player_id,
            "quest_id": progress.quest_id,
            "step_id": progress.step_id,
            "room_id": progress.room_id,
            "visited_count": progress.visited_count,
            "total": progress.total,
            "complete": progress.complete,
        });
        // Private update to the questing player.
        send_game_private(state, my_key, &payload).await;
        // Public broadcast on completion (so other players can cheer).
        if progress.complete {
            let _ = state.broadcast_tx.send(RelayMessage::System {
                message: format!("__game__:{}", payload),
            });
            // Order matters: apply_quest_reward reads the current_quest
            // (still explore_ship) → fires reward → chain_next_quest swaps
            // it to meet_the_crew → unlock event surfaces the new quest.
            let mut world = state.game_world.write().await;
            let reward = world.apply_quest_reward(player_id);
            let next_quest = world.chain_next_quest(player_id);
            drop(world);
            // Durably persist the player's new progress (xp/reputation bumped,
            // completed_quests appended, current_quest advanced) so it survives
            // a relay restart. Best-effort; never blocks the reward broadcast.
            persist_player_progress(state, my_key, player_id).await;
            if let Some(r) = reward {
                let reward_payload = serde_json::json!({
                    "type": "game_quest_reward",
                    "player_id": player_id,
                    "quest_id": r.quest_id,
                    "xp": r.xp,
                    "reputation": r.reputation,
                    "message": r.message,
                    "xp_total": r.xp_total,
                    "reputation_total": r.reputation_total,
                });
                send_game_private(state, my_key, &reward_payload).await;
            }
            if let Some(q) = next_quest {
                let unlock = serde_json::json!({
                    "type": "game_quest_unlocked",
                    "player_id": player_id,
                    "quest": q,
                });
                send_game_private(state, my_key, &unlock).await;
            }
        }
    }

    // NPC greeting on first entry into a room (v0.169.0). Sent privately
    // so it doesn't spam other players, and only fires the first time the
    // player enters that room (gated by `quest_progress.is_some()` above).
    if let Some((speaker, line)) = greeting {
        let payload = serde_json::json!({
            "type": "game_npc_greeting",
            "player_id": player_id,
            "speaker": speaker,
            "line": line,
        });
        send_game_private(state, my_key, &payload).await;
    }
}

/// Handle a game client disconnecting. Removes their player entity and broadcasts PlayerLeft.
pub async fn handle_game_disconnect(
    state: &Arc<RelayState>,
    player_key: &str,
) {
    let mut world = state.game_world.write().await;
    // Capture the player's progress BEFORE despawning (despawn removes the
    // entity, so we can't read it afterward). Find the entity, snapshot its
    // progression, then despawn.
    let progress = world
        .find_player_entity(player_key)
        .and_then(|id| world.extract_player_progress(id));
    if let Some(entity_id) = world.despawn_player(player_key) {
        drop(world);

        // Durably persist the leaving player's progress so a returning player
        // resumes their quest/XP even if the periodic world snapshot hadn't
        // captured this session yet. Best-effort; failure is logged only.
        if let Some((current_quest, completed, xp, reputation)) = progress {
            if let Err(e) = state.db.save_player_progress(
                player_key,
                current_quest.as_deref(),
                &completed,
                xp,
                reputation,
            ) {
                tracing::warn!("Could not persist progress on disconnect for {}: {e}", player_key);
            }
        }

        let left = serde_json::json!({
            "type": "game_player_left",
            "player_id": entity_id,
        });
        let _ = state.broadcast_tx.send(RelayMessage::System {
            message: format!("__game__:{}", left),
        });

        tracing::info!("Game: player {} left (entity {})", player_key, entity_id);
    }
}

// ── Perception API rate limiting ──

/// Minimum interval between calls of the SAME action type per public key
/// (200ms = 5 calls/sec). Each action (perceive / interact / query_inventory
/// / query_entity) has its own bucket so a "perceive → interact → perceive"
/// flow isn't blocked by a single shared limit. Bucket key = `pubkey|action`.
const PERCEPTION_MIN_INTERVAL_MS: u64 = 200;

/// Returns true if the caller is allowed to make this action right now,
/// false if they need to wait. On false, we send a Private rate-limit warning.
/// `action` is "perceive" / "interact" / "query_inventory" / "query_entity".
fn check_perception_rate(state: &Arc<RelayState>, my_key: &str, action: &str) -> bool {
    let now = std::time::Instant::now();
    let bucket = format!("{}|{}", my_key, action);
    let allowed = {
        let mut map = match state.last_perception_times.lock() {
            Ok(m) => m,
            Err(p) => p.into_inner(),
        };
        let allowed = match map.get(&bucket) {
            Some(last) => now.duration_since(*last).as_millis() as u64 >= PERCEPTION_MIN_INTERVAL_MS,
            None => true,
        };
        if allowed {
            map.insert(bucket, now);
        }
        allowed
    };
    if !allowed {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: format!(
                "__game__:{{\"type\":\"game_error\",\"error\":\"rate_limited\",\"action\":\"{}\",\"message\":\"{} calls are limited to {} per second.\"}}",
                action, action,
                1000 / PERCEPTION_MIN_INTERVAL_MS
            ),
        };
        let _ = state.broadcast_tx.send(private);
    }
    allowed
}

// ── Perception API handlers ──

/// Handle a perception query. Returns the player's surroundings as structured JSON:
/// current room, nearby entities, environment state, and player stats.
pub async fn handle_game_perceive(
    state: &Arc<RelayState>,
    my_key: &str,
    raw: &serde_json::Value,
) {
    if !check_perception_rate(state, my_key, "perceive") { return; }
    let radius = raw.get("radius")
        .and_then(|v| v.as_f64())
        .unwrap_or(20.0)
        .min(100.0) as f32;

    let world = state.game_world.read().await;

    let player_id = match world.find_player_entity(my_key) {
        Some(id) => id,
        None => {
            send_game_private(state, my_key, &serde_json::json!({
                "type": "game_error",
                "error": "not_in_game",
                "message": "Send game_join first",
            })).await;
            return;
        }
    };

    let player = &world.entities[&player_id];
    let position = player.position;

    let location = world.room_for_position(position);
    let nearby = world.entities_near(position, radius);

    // Filter out self from nearby list.
    let nearby_filtered: Vec<_> = nearby.into_iter()
        .filter(|e| e.entity_id != player_id)
        .collect();

    let response = serde_json::json!({
        "type": "game_perception",
        "position": position,
        "location": location,
        "nearby_entities": nearby_filtered,
        "environment": {
            "game_time": world.game_time,
            "ship": world.ship_name,
            "orbit": "Earth LEO, 400km altitude",
        },
        "player": {
            "entity_id": player_id,
            "health": player.components.get("health"),
            "stamina": player.components.get("stamina"),
        },
    });

    drop(world);
    send_game_private(state, my_key, &response).await;
}

/// Handle an interaction with a game entity. Validates distance and returns
/// entity component data as the interaction result.
pub async fn handle_game_interact(
    state: &Arc<RelayState>,
    my_key: &str,
    raw: &serde_json::Value,
) {
    if !check_perception_rate(state, my_key, "interact") { return; }
    let entity_id = match raw.get("entity_id").and_then(|v| v.as_u64()) {
        Some(id) => id,
        None => return,
    };
    let action = raw.get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("inspect")
        .to_string();

    let world = state.game_world.read().await;

    let player_id = match world.find_player_entity(my_key) {
        Some(id) => id,
        None => {
            send_game_private(state, my_key, &serde_json::json!({
                "type": "game_error",
                "error": "not_in_game",
                "message": "Send game_join first",
            })).await;
            return;
        }
    };

    let player_pos = world.entities[&player_id].position;

    let target = match world.entities.get(&entity_id) {
        Some(e) => e,
        None => {
            drop(world);
            send_game_private(state, my_key, &serde_json::json!({
                "type": "game_interact_result",
                "entity_id": entity_id,
                "action": action,
                "success": false,
                "error": "entity_not_found",
            })).await;
            return;
        }
    };

    let dx = target.position[0] - player_pos[0];
    let dy = target.position[1] - player_pos[1];
    let dz = target.position[2] - player_pos[2];
    let dist = (dx * dx + dy * dy + dz * dz).sqrt();

    if dist > 5.0 {
        drop(world);
        send_game_private(state, my_key, &serde_json::json!({
            "type": "game_interact_result",
            "entity_id": entity_id,
            "action": action,
            "success": false,
            "error": "too_far",
            "distance": dist,
        })).await;
        return;
    }

    let interactable = target.components.get("interactable")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !interactable {
        drop(world);
        send_game_private(state, my_key, &serde_json::json!({
            "type": "game_interact_result",
            "entity_id": entity_id,
            "action": action,
            "success": false,
            "error": "not_interactable",
        })).await;
        return;
    }

    // If the entity has a `dialog` array (NPC), pick a random line and
    // include it as `dialog_line` in the response. The action "talk" always
    // gets a line; other actions get one too — keeps the API forgiving for
    // AI agents experimenting with verbs.
    let dialog_line: Option<String> = target.components.get("dialog")
        .and_then(|v| v.as_array())
        .filter(|arr| !arr.is_empty())
        .map(|arr| {
            use rand::Rng;
            let idx = rand::thread_rng().gen_range(0..arr.len());
            arr[idx].as_str().unwrap_or("").to_string()
        })
        .filter(|s| !s.is_empty());

    let speaker_name: Option<String> = target.components.get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mut response = serde_json::json!({
        "type": "game_interact_result",
        "entity_id": entity_id,
        "entity_type": target.entity_type,
        "action": action,
        "success": true,
        "components": target.components,
        "position": target.position,
    });
    if let Some(line) = &dialog_line {
        response["dialog_line"] = serde_json::Value::String(line.clone());
    }
    if let Some(name) = &speaker_name {
        response["speaker"] = serde_json::Value::String(name.clone());
    }

    drop(world);
    send_game_private(state, my_key, &response).await;

    // Broadcast the interaction to other clients. If a dialog line was
    // returned, surface it in the broadcast too — humans nearby will see
    // "Helm Officer Vex: Course laid in." style chat.
    let mut broadcast = serde_json::json!({
        "type": "game_entity_interacted",
        "entity_id": entity_id,
        "player_key": my_key,
        "action": action,
    });
    if let Some(line) = dialog_line {
        broadcast["dialog_line"] = serde_json::Value::String(line);
    }
    if let Some(ref name) = speaker_name {
        broadcast["speaker"] = serde_json::Value::String(name.clone());
    }
    let _ = state.broadcast_tx.send(RelayMessage::System {
        message: format!("__game__:{}", broadcast),
    });

    // Quest progress for the meet_the_crew (NPC speaker) AND survey_storage
    // (storage entity) flows. record_npc_talk fires only when interacting
    // with a named NPC; record_storage_scan fires only when interacting
    // with a storage entity. Both are mutually exclusive in practice but
    // the handler runs both checks so verbs stay forgiving.
    {
        let mut world = state.game_world.write().await;
        let mut progress: Option<crate::relay::handlers::game_state::QuestProgress> = None;
        if let Some(ref npc_name) = speaker_name {
            progress = world.record_npc_talk(player_id, npc_name);
        }
        if progress.is_none() {
            progress = world.record_storage_scan(player_id, entity_id);
        }
        // Apply reward + chain ONLY when the action completed the quest.
        let reward_and_next = if progress.as_ref().map_or(false, |p| p.complete) {
            let r = world.apply_quest_reward(player_id);
            let n = world.chain_next_quest(player_id);
            Some((r, n))
        } else {
            None
        };
        drop(world);
        // If this interaction completed a quest, the player's xp/reputation/
        // current_quest just changed — persist it durably so it survives a
        // relay restart. Best-effort; never blocks the broadcasts below.
        if reward_and_next.is_some() {
            persist_player_progress(state, my_key, player_id).await;
        }
        if let Some(progress) = progress {
            let event_type = if progress.complete {
                "game_quest_completed"
            } else {
                "game_quest_progress"
            };
            let payload = serde_json::json!({
                "type": event_type,
                "player_id": player_id,
                "quest_id": progress.quest_id,
                "step_id": progress.step_id,
                "room_id": progress.room_id,
                "visited_count": progress.visited_count,
                "total": progress.total,
                "complete": progress.complete,
            });
            send_game_private(state, my_key, &payload).await;
            if progress.complete {
                let _ = state.broadcast_tx.send(RelayMessage::System {
                    message: format!("__game__:{}", payload),
                });
            }
        }
        if let Some((reward, next)) = reward_and_next {
            if let Some(r) = reward {
                let reward_payload = serde_json::json!({
                    "type": "game_quest_reward",
                    "player_id": player_id,
                    "quest_id": r.quest_id,
                    "xp": r.xp,
                    "reputation": r.reputation,
                    "message": r.message,
                    "xp_total": r.xp_total,
                    "reputation_total": r.reputation_total,
                });
                send_game_private(state, my_key, &reward_payload).await;
            }
            if let Some(q) = next {
                let unlock = serde_json::json!({
                    "type": "game_quest_unlocked",
                    "player_id": player_id,
                    "quest": q,
                });
                send_game_private(state, my_key, &unlock).await;
            }
        }
    }
}

/// Handle an inventory query. Returns the player's inventory component.
pub async fn handle_game_query_inventory(
    state: &Arc<RelayState>,
    my_key: &str,
) {
    if !check_perception_rate(state, my_key, "query_inventory") { return; }
    let world = state.game_world.read().await;

    let player_id = match world.find_player_entity(my_key) {
        Some(id) => id,
        None => {
            send_game_private(state, my_key, &serde_json::json!({
                "type": "game_error",
                "error": "not_in_game",
                "message": "Send game_join first",
            })).await;
            return;
        }
    };

    let player = &world.entities[&player_id];
    let inventory = player.components.get("inventory")
        .cloned()
        .unwrap_or(serde_json::json!([]));

    let response = serde_json::json!({
        "type": "game_inventory",
        "player_id": player_id,
        "inventory": inventory,
    });

    drop(world);
    send_game_private(state, my_key, &response).await;
}

/// Handle an entity detail query. Returns full entity snapshot if within
/// perception range (20m).
pub async fn handle_game_query_entity(
    state: &Arc<RelayState>,
    my_key: &str,
    raw: &serde_json::Value,
) {
    if !check_perception_rate(state, my_key, "query_entity") { return; }
    let entity_id = match raw.get("entity_id").and_then(|v| v.as_u64()) {
        Some(id) => id,
        None => return,
    };

    let world = state.game_world.read().await;

    let player_id = match world.find_player_entity(my_key) {
        Some(id) => id,
        None => {
            send_game_private(state, my_key, &serde_json::json!({
                "type": "game_error",
                "error": "not_in_game",
                "message": "Send game_join first",
            })).await;
            return;
        }
    };

    let player_pos = world.entities[&player_id].position;

    let target = match world.entities.get(&entity_id) {
        Some(e) => e,
        None => {
            drop(world);
            send_game_private(state, my_key, &serde_json::json!({
                "type": "game_entity_detail",
                "entity_id": entity_id,
                "found": false,
            })).await;
            return;
        }
    };

    let dx = target.position[0] - player_pos[0];
    let dy = target.position[1] - player_pos[1];
    let dz = target.position[2] - player_pos[2];
    let dist = (dx * dx + dy * dy + dz * dz).sqrt();

    if dist > 20.0 {
        drop(world);
        send_game_private(state, my_key, &serde_json::json!({
            "type": "game_entity_detail",
            "entity_id": entity_id,
            "found": false,
            "error": "out_of_range",
        })).await;
        return;
    }

    let response = serde_json::json!({
        "type": "game_entity_detail",
        "entity_id": entity_id,
        "found": true,
        "entity_type": target.entity_type,
        "position": target.position,
        "rotation": target.rotation,
        "distance": dist,
        "components": target.components,
        "owner": target.owner,
    });

    drop(world);
    send_game_private(state, my_key, &response).await;
}

/// Send a game message privately to one player.
async fn send_game_private(state: &Arc<RelayState>, to_key: &str, msg: &serde_json::Value) {
    let private = RelayMessage::Private {
        to: to_key.to_string(),
        message: format!("__game__:{}", msg),
    };
    let _ = state.broadcast_tx.send(private);
}

/// Persist a player's current progression (quest / completed list / xp /
/// reputation) to durable storage. Call this whenever a player's progress
/// changes meaningfully — i.e. after a quest reward is applied / the quest
/// chain advances, and on disconnect — so it survives a relay restart.
///
/// Reads the player entity under a fresh read lock, so callers should invoke
/// this AFTER dropping any write lock they hold. Best-effort: a DB error is
/// logged, not propagated (a failed progress save must never break gameplay).
async fn persist_player_progress(state: &Arc<RelayState>, player_key: &str, player_id: u64) {
    let extracted = {
        let world = state.game_world.read().await;
        world.extract_player_progress(player_id)
    };
    if let Some((current_quest, completed, xp, reputation)) = extracted {
        if let Err(e) = state.db.save_player_progress(
            player_key,
            current_quest.as_deref(),
            &completed,
            xp,
            reputation,
        ) {
            tracing::warn!("Could not persist player progress for {}: {e}", player_key);
        }
    }
}

// ── handle_mod_action handler-layer tests (v0.250 — regression guards
//    for the v0.245 ban / v0.246 mute / v0.247 name-only-bypass work).
//    RelayState::new(db) needs only a Storage + no network, so the whole
//    handler is testable; we subscribe to broadcast_tx to capture the
//    Private/System feedback it emits. tokio has rt-multi-thread but not
//    `macros`, so we drive the async fn via a manual Runtime. ──
#[cfg(test)]
mod mod_action_tests {
    use super::*;
    use crate::relay::relay::RelayState;

    fn fresh_state() -> Arc<RelayState> {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_modact_{pid}_{nanos}.db"));
        let db = Storage::open(&path).expect("open test db");
        Arc::new(RelayState::new(db))
    }

    /// Collect the Private/System message bodies a handler broadcast.
    fn drain(rx: &mut tokio::sync::broadcast::Receiver<RelayMessage>) -> Vec<String> {
        let mut out = Vec::new();
        loop {
            match rx.try_recv() {
                Ok(RelayMessage::Private { message, .. }) => out.push(message),
                Ok(RelayMessage::System { message }) => out.push(message),
                Ok(_) => {}
                Err(_) => break,
            }
        }
        out
    }

    fn block<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Runtime::new().expect("tokio rt").block_on(f)
    }

    /// A caller with no mod/admin role can't perform any mod action.
    #[test]
    fn non_mod_cannot_act() {
        let st = fresh_state();
        st.db.register_name("Victim", "victim_key").unwrap();
        st.db.join_server("victim_key", "Victim").unwrap();
        let mut rx = st.broadcast_tx.subscribe();
        block(handle_mod_action(&st, "rando_key", "kick", "victim_key", "Victim"));
        let msgs = drain(&mut rx);
        assert!(
            msgs.iter().any(|m| m.contains("don't have permission")),
            "expected permission denial, got {msgs:?}"
        );
        assert!(
            !st.db.keys_for_name("Victim").unwrap().is_empty(),
            "victim must not have been kicked"
        );
    }

    /// Ban is admin-only — a plain moderator is refused.
    #[test]
    fn mod_cannot_ban_admin_only() {
        let st = fresh_state();
        st.db.set_role("mod_key", "mod").unwrap();
        st.db.register_name("Victim", "victim_key").unwrap();
        let mut rx = st.broadcast_tx.subscribe();
        block(handle_mod_action(&st, "mod_key", "ban", "victim_key", "Victim"));
        let msgs = drain(&mut rx);
        assert!(
            msgs.iter().any(|m| m.contains("requires admin")),
            "ban must be admin-only, got {msgs:?}"
        );
        assert!(!st.db.is_banned("victim_key").unwrap());
    }

    /// Admin ban happy path: persists to banned_keys WITH the display
    /// name (the v0.245 irreversible-ban-trap guard, end-to-end through
    /// the handler) and confirms to the actor.
    #[test]
    fn admin_ban_persists_and_captures_name() {
        let st = fresh_state();
        st.db.set_role("admin_key", "admin").unwrap();
        st.db.register_name("Victim", "victim_key").unwrap();
        st.db.join_server("victim_key", "Victim").unwrap();
        st.db.set_role("victim_key", "verified").unwrap();
        let mut rx = st.broadcast_tx.subscribe();
        block(handle_mod_action(&st, "admin_key", "ban", "victim_key", "Victim"));
        let msgs = drain(&mut rx);
        assert!(st.db.is_banned("victim_key").unwrap(), "victim must be banned");
        let banned = st.db.list_banned().unwrap();
        assert_eq!(banned.len(), 1);
        assert_eq!(
            banned[0].name, "Victim",
            "name must be captured for the Banned-users panel"
        );
        assert!(
            msgs.iter().any(|m| m.contains("Banned")),
            "actor should get a Banned confirmation, got {msgs:?}"
        );
    }

    /// v0.247 SECURITY regression: a non-admin moderator must NOT be
    /// able to act on an admin via the NAME-ONLY path (no target key).
    /// Before v0.247 the admin-protection check needed a key, so this
    /// slipped straight through.
    #[test]
    fn name_only_kick_of_admin_is_refused() {
        let st = fresh_state();
        st.db.set_role("mod_key", "mod").unwrap();
        st.db.register_name("BigBoss", "boss_key").unwrap();
        st.db.join_server("boss_key", "BigBoss").unwrap();
        st.db.set_role("boss_key", "admin").unwrap();
        let mut rx = st.broadcast_tx.subscribe();
        // target key empty — only the name is supplied.
        block(handle_mod_action(&st, "mod_key", "kick", "", "BigBoss"));
        let msgs = drain(&mut rx);
        assert!(
            msgs.iter()
                .any(|m| m.contains("Only an admin can act on another admin")),
            "name-only kick of an admin must be refused, got {msgs:?}"
        );
        assert!(
            !st.db.keys_for_name("BigBoss").unwrap().is_empty(),
            "the admin must NOT have been kicked via the name-only path"
        );
    }

    /// Self-protection also covers the name-only path (v0.247 widened
    /// the self-guard from `target == my_key` to the resolved key set).
    #[test]
    fn cannot_self_ban_by_name() {
        let st = fresh_state();
        st.db.set_role("me_key", "admin").unwrap();
        st.db.register_name("Me", "me_key").unwrap();
        let mut rx = st.broadcast_tx.subscribe();
        block(handle_mod_action(&st, "me_key", "ban", "", "Me"));
        let msgs = drain(&mut rx);
        assert!(
            msgs.iter().any(|m| m.contains("can't ban yourself")),
            "self-ban by name must be blocked, got {msgs:?}"
        );
        assert!(!st.db.is_banned("me_key").unwrap());
    }

    /// v0.246 invariant proven through the handler: mute must not touch
    /// the user's role, and unmute leaves it intact (a Donor stays a
    /// Donor across mute → unmute).
    #[test]
    fn mute_preserves_role_through_handler() {
        let st = fresh_state();
        st.db.set_role("mod_key", "mod").unwrap();
        st.db.register_name("Donor", "donor_key").unwrap();
        st.db.set_role("donor_key", "donor").unwrap();

        block(handle_mod_action(&st, "mod_key", "mute", "donor_key", "Donor"));
        assert!(st.db.is_muted("donor_key").unwrap(), "should be muted");
        assert_eq!(
            st.db.get_role("donor_key").unwrap(),
            "donor",
            "mute must not clobber the role"
        );

        block(handle_mod_action(&st, "mod_key", "unmute", "donor_key", "Donor"));
        assert!(!st.db.is_muted("donor_key").unwrap(), "should be unmuted");
        assert_eq!(
            st.db.get_role("donor_key").unwrap(),
            "donor",
            "role still intact after unmute"
        );
    }
}

// ── Livestream handler tests (v0.645, overnight-loop priority #2 verification) ──
#[cfg(test)]
mod stream_tests {
    use super::*;
    use crate::relay::relay::RelayState;

    fn fresh_state() -> Arc<RelayState> {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_stream_{pid}_{nanos}.db"));
        let db = Storage::open(&path).expect("open test db");
        Arc::new(RelayState::new(db))
    }

    fn block<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Runtime::new().expect("tokio rt").block_on(f)
    }

    /// Enable the server-wide streaming switch (off by default) and grant the
    /// streamer's role can_stream (mirrors the real admin/mod seed defaults).
    fn allow_streaming(st: &Arc<RelayState>, streamer_key: &str) {
        st.db.set_role(streamer_key, "admin").unwrap();
        let mut settings = st.db.get_server_settings().unwrap_or_default();
        settings.video_streaming_enabled = true;
        st.db.set_server_settings(&settings, streamer_key).unwrap();
    }

    /// The exact bug this test guards: `viewer_peak` used to be fed the LIVE
    /// `viewer_keys.len()` at leave/stop time, which is only ever highest
    /// right at a join and monotonically decreases from there -- so by the
    /// time a stream actually ends (viewers usually already trickled out),
    /// the persisted peak was frequently 0 or far below the real maximum.
    /// Two viewers join (peak momentarily 2), both leave (live count back to
    /// 0), THEN the stream stops -- the persisted viewer_peak must still be
    /// the true historical 2, not the live-at-stop-time 0.
    #[test]
    fn viewer_peak_survives_viewers_leaving_before_stream_stops() {
        let st = fresh_state();
        allow_streaming(&st, "streamer_key");

        block(handle_stream_start(&st, "streamer_key", "Test Stream".to_string(), "testing".to_string()));
        block(handle_stream_viewer_join(&st, "viewer1_key"));
        block(handle_stream_viewer_join(&st, "viewer2_key"));
        // Peak is 2 right now -- both viewers leave before anyone checks.
        block(handle_stream_viewer_leave(&st, "viewer1_key"));
        block(handle_stream_viewer_leave(&st, "viewer2_key"));
        block(handle_stream_stop(&st, "streamer_key"));

        let streams = st.db.get_recent_streams(1).unwrap();
        assert_eq!(streams.len(), 1, "expected exactly one recorded stream");
        let (_, streamer_key, _, _, _, ended_at, viewer_peak) = &streams[0];
        assert_eq!(streamer_key, "streamer_key");
        assert!(ended_at.is_some(), "stream_stop must set ended_at");
        assert_eq!(*viewer_peak, 2, "the persisted peak must be the true historical max (2), not the live count at stop time (0)");
    }

    /// A single viewer who joins then leaves (peak 1) must not be recorded
    /// as 0 just because they left before the stream ended -- the simplest
    /// case of the same bug class, kept separate so a regression here is
    /// unambiguous about which scenario broke.
    #[test]
    fn viewer_peak_of_one_is_not_lost_when_that_viewer_leaves() {
        let st = fresh_state();
        allow_streaming(&st, "streamer_key");

        block(handle_stream_start(&st, "streamer_key", "Solo Viewer Stream".to_string(), "testing".to_string()));
        block(handle_stream_viewer_join(&st, "viewer1_key"));
        block(handle_stream_viewer_leave(&st, "viewer1_key"));
        block(handle_stream_stop(&st, "streamer_key"));

        let streams = st.db.get_recent_streams(1).unwrap();
        let (_, _, _, _, _, _, viewer_peak) = &streams[0];
        assert_eq!(*viewer_peak, 1);
    }

    /// A stream nobody ever watches must record a peak of 0, not error or
    /// panic -- the zero-viewer path through the same code.
    #[test]
    fn a_stream_with_no_viewers_records_zero_peak() {
        let st = fresh_state();
        allow_streaming(&st, "streamer_key");

        block(handle_stream_start(&st, "streamer_key", "Empty Room".to_string(), "testing".to_string()));
        block(handle_stream_stop(&st, "streamer_key"));

        let streams = st.db.get_recent_streams(1).unwrap();
        let (_, _, _, _, _, _, viewer_peak) = &streams[0];
        assert_eq!(*viewer_peak, 0);
    }

    /// Streaming is refused when the server-wide switch is off, even for an
    /// admin -- the master-kill-switch half of the authorization check
    /// (`may_stream = settings.video_streaming_enabled && rd.can_stream`).
    #[test]
    fn streaming_disabled_server_wide_blocks_even_an_admin() {
        let st = fresh_state();
        st.db.set_role("streamer_key", "admin").unwrap();
        // Deliberately do NOT enable video_streaming_enabled.
        let mut rx = st.broadcast_tx.subscribe();
        block(handle_stream_start(&st, "streamer_key", "Should Not Start".to_string(), "testing".to_string()));
        let mut saw_refusal = false;
        while let Ok(msg) = rx.try_recv() {
            if let RelayMessage::Private { message, .. } = msg {
                if message.contains("disabled server-wide") {
                    saw_refusal = true;
                }
            }
        }
        assert!(saw_refusal, "expected a server-wide-disabled refusal message");
        assert_eq!(st.db.get_recent_streams(10).unwrap().len(), 0, "no stream row should be created");
    }
}
