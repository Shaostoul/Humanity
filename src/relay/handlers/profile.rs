//! The profile handlers: a person's save (`profile_update`), a request for a profile
//! (`profile_request`), and the copy of their profile that other servers keep, which their own
//! app signs (`profile_sign`, BUG-191).
//!
//! Moved out of msg_handlers.rs on 2026-10-12 (docs/design/bug-191-profile-copies-plan.md,
//! Wave 0) to make room under the file-size ratchet for asking for that copy and checking it. The
//! two handlers moved verbatim, but for the two new own-copy fields (`signed_at`, `sign_now`) and
//! the save's unsigned copy for other servers, which is gone.

use std::sync::Arc;
use std::time::Instant;

use crate::relay::core::profile_copy::{PublicCopy, BIO_MAX_CHARS};
use crate::relay::handlers::friend_passes::friend_pass;
use crate::relay::relay::{RelayMessage, RelayState};

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
    accepts_files: Option<String>,
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

        // Counted in characters, as the words say: counted in bytes until 2026-10-11, which
        // refused a bio in Greek, Hindi or Japanese, or with emoji, well under 280 characters.
        if clean_bio.chars().count() > BIO_MAX_CHARS {
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

        // Who they accept files in messages from (media-showcase.md, 2026-10-11): a word that is
        // not one of the three REFUSES the whole update, as an unknown audience refuses a whole
        // `reach_set` (handlers/reach.rs): saving the rest would leave the person believing their
        // choice was kept, and dropping the word quietly would hide a broken app. Not sent at all
        // (an older app) keeps the stored word (storage/profile.rs `save_profile_extended`).
        if let Some(word) = accepts_files.as_deref() {
            if crate::relay::storage::profile::accepts_files_word(word).is_none() {
                let shown: String = word.chars().take(40).collect();
                let _ = state.broadcast_tx.send(RelayMessage::Private {
                    to: my_key.to_string(),
                    message: format!(
                        "Your profile was not saved: \"{shown}\" is not one of nobody, friends, anyone (who you accept files from)."
                    ),
                });
                return;
            }
        }

        match state.db.save_profile_extended(
            name, &clean_bio, &socials, &avatar, &banner, &pronoun, &loc, &w_site, &priv_map,
            accepts_files.as_deref(),
        ) {
            Ok(accepts_files_stored) => {
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
                    accepts_files: Some(accepts_files_stored.clone()),
                    privacy: None,
                    stored: None,
                    signed_at: None,
                    sign_now: None,
                    target: None,
                });

                // The person's own copy goes to every device signed in under this name: a linked
                // device has a key of its own (a link code adds one under the name), and each needs
                // the privacy map to send the whole profile back without undoing this save.
                let mut own_keys = state.db.keys_for_name(name).unwrap_or_default();
                if !own_keys.iter().any(|k| k == my_key) {
                    own_keys.push(my_key.to_string());
                }
                for key in own_keys {
                    let _ = state.broadcast_tx.send(RelayMessage::ProfileData {
                        name: name.clone(),
                        bio: clean_bio.clone(),
                        socials: socials.clone(),
                        avatar_url: if avatar.is_empty() { None } else { Some(avatar.clone()) },
                        banner_url: if banner.is_empty() { None } else { Some(banner.clone()) },
                        pronouns: if pronoun.is_empty() { None } else { Some(pronoun.clone()) },
                        location: if loc.is_empty() { None } else { Some(loc.clone()) },
                        website: if w_site.is_empty() { None } else { Some(w_site.clone()) },
                        accepts_files: Some(accepts_files_stored.clone()),
                        privacy: Some(priv_map.clone()),
                        stored: Some(true),
                        signed_at: None,
                        sign_now: None,
                        target: Some(key),
                    });
                }

                // The save no longer writes the copy kept for other servers (BUG-191): nothing signed it,
                // so every linked server refused it, and before 2026-10-11 it held fields kept for friends.
                // The person's own app signs the public copy after this echo (handle_profile_sign). Until
                // the signed withdrawal (Wave 1A), going unlisted still removes this server's own copy.
                let unlisted = privacy
                    .as_deref()
                    .and_then(|p| serde_json::from_str::<serde_json::Value>(p).ok())
                    .and_then(|v| v.get("directory").and_then(|d| d.as_str()).map(|s| s == "unlisted"))
                    .unwrap_or(false);
                if unlisted {
                    let _ = state.db.delete_signed_profile(my_key);
                }
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
    friend_cert: Option<String>,
) {
    // Follows-graph removal (2026-08-24): friends-visibility profile
    // fields unlock by presenting the profile OWNER's friendship pass
    // (v2 since 2026-10-09, handlers/friend_passes.rs), checked without a
    // friends table; a withdrawn pass counts as none.
    let owner_key = {
        let peers = state.peers.read().await;
        peers.values()
            .find(|p| p.display_name.as_deref().map(|n| n.eq_ignore_ascii_case(&name)).unwrap_or(false))
            .map(|p| p.public_key_hex.clone())
            .unwrap_or_default()
    };
    let is_friend = !owner_key.is_empty() && friend_pass(state, &owner_key, my_key, friend_cert.as_deref()).is_some();

    // The person asking for their OWN profile (an app that has not had its own copy from this
    // server, such as one switched to without a fresh connect) gets their complete copy, privacy
    // map included, as on connect; an empty one when they have no profile here yet.
    if state.db.keys_for_name(&name).map(|keys| keys.iter().any(|k| k == my_key)).unwrap_or(false) {
        // A profile that could not be read is not answered: an empty copy would be taken as the
        // server's, and the app's next save would wipe the profile (no row is an empty profile).
        let row = match state.db.get_profile_extended(&name) {
            Ok(row) => row,
            Err(e) => {
                tracing::error!("Failed to read own profile: {e}");
                return;
            }
        };
        let stored = row.is_some();
        let (bio, socials, avatar_url, banner_url, pronouns, location, website, privacy) = row.unwrap_or_default();
        let opt = |s: String| if s.is_empty() { None } else { Some(s) };
        let _ = state.broadcast_tx.send(RelayMessage::ProfileData {
            name: name.clone(),
            bio,
            socials: if socials.trim().is_empty() { "{}".to_string() } else { socials },
            avatar_url: opt(avatar_url),
            banner_url: opt(banner_url),
            pronouns: opt(pronouns),
            location: opt(location),
            website: opt(website),
            accepts_files: Some(state.db.accepts_files_of(&name)),
            privacy: Some(if privacy.trim().is_empty() { "{}".to_string() } else { privacy }),
            stored: Some(stored),
            signed_at: None,
            sign_now: None,
            target: Some(my_key.to_string()),
        });
        return;
    }

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
                accepts_files: get("accepts_files"),
                privacy: None,
                stored: None,
                signed_at: None,
                sign_now: None,
                target: Some(my_key.to_string()),
            });
        }
        Ok(None) => {
            // No profile row: they never chose, so the default is what their app enforces.
            let _ = state.broadcast_tx.send(RelayMessage::ProfileData {
                name: name.clone(),
                bio: String::new(),
                socials: "{}".to_string(),
                avatar_url: None,
                banner_url: None,
                pronouns:   None,
                location:   None,
                website:    None,
                accepts_files: Some(crate::relay::storage::profile::ACCEPTS_FILES_DEFAULT.to_string()),
                privacy: None,
                stored: None,
                signed_at: None,
                sign_now: None,
                target: Some(my_key.to_string()),
            });
        }
        Err(e) => {
            tracing::error!("Failed to get profile: {e}");
        }
    }
}

// ── The copy for other servers (BUG-191) ──

/// What a person's own copy says about the copy for other servers: the time of the one this
/// server holds for the receiving key (`signed_at`, 0 for none) and whether this server asks that
/// device to sign one now (`sign_now`). Both ride the own copy (`ProfileData`) only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CopyAsk {
    pub signed_at: u64,
    pub sign_now: bool,
}

/// Whether to ask `key`'s device to sign the public copy `now` (the copy of what this server
/// stored, under `privacy`), as `profile_copy::sign_wanted` decides against the copy held here;
/// the ask is recorded in `RelayState::profile_sign_asks`, and never made for a key that is not a
/// person's (bots). `just_saved`: the echo of a save (ask whenever the copy changed), rather than
/// a connect or the person's own request (ask only to narrow what is out there).
///
/// WAVE 0 STUB: answers "no copy held, no ask". Wave 1A fills it in (plan 3.3).
pub fn ask_for_copy(state: &RelayState, key: &str, now: &PublicCopy, privacy: &str, just_saved: bool) -> CopyAsk {
    let _ = (state, key, now, privacy, just_saved);
    CopyAsk::default()
}

/// A device's signature over its profile's copy for other servers (`profile_sign`): checked
/// against the copy rebuilt from what this server stored, then kept and passed on (plan 3.5).
///
/// WAVE 0 STUB: logs and drops it. Wave 1A fills it in.
pub async fn handle_profile_sign(state: &Arc<RelayState>, my_key: &str, at: u64, signature: String) {
    let _ = (state, signature);
    tracing::debug!(
        "Profile: dropped a copy for other servers from {} (at {at}): not checked yet",
        crate::relay::handlers::federation::key_in_log(my_key)
    );
}

/// The profile handlers' tests: the field `accepts_files` (2026-10-11, docs/design/media-showcase.md),
/// the person's own copy, and what the lookups show.
#[cfg(test)]
#[path = "profile_tests.rs"]
mod profile_tests;
