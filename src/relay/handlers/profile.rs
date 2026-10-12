//! The profile handlers: a person's save (`profile_update`), a request for a profile
//! (`profile_request`), and the copy of their profile that other servers keep, which their own
//! app signs (`profile_sign`, BUG-191).
//!
//! Moved out of msg_handlers.rs on 2026-10-12 (docs/design/bug-191-profile-copies-plan.md,
//! Wave 0) to make room under the file-size ratchet for asking for that copy and checking it.
//!
//! THE COPY FOR OTHER SERVERS (BUG-191, Wave 1A, plan 3.3 and 3.5). The save stores no copy
//! itself. Its echo, the person's own complete copy sent to each of their devices, asks each one
//! to sign the public copy of what was stored (`sign_now`, decided by `ask_for_copy`); a device
//! that was asked answers with `profile_sign`, which `handle_profile_sign` checks against the copy
//! it rebuilds from what it stored, keeps, and passes on to the servers this one is linked with.
//! Going unlisted is the same path: the public copy is then the withdrawal, which replaces every
//! copy already passed on.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::relay::core::profile_copy::{self, public_copy, PublicCopy, BIO_MAX_CHARS, MAX_AHEAD_MS};
use crate::relay::handlers::federation::{gossip_profile, key_in_log};
use crate::relay::handlers::friend_passes::friend_pass;
use crate::relay::relay::{RelayMessage, RelayState};

/// How long an ask to sign stays open (plan 3.5 step 1): a `profile_sign` from a key this server
/// did not ask within the last minute is dropped, so a device cannot push copies nobody asked for.
pub const ASK_LIFETIME: Duration = Duration::from_secs(60);

/// What a person is told when their device dated its copy more than a day ahead of this server's
/// clock: the one refusal they can fix themselves (plan 3.5 step 6).
pub const CLOCK_AHEAD_NOTICE: &str = "Your profile is saved on this server, but it was not passed on to the servers this one is linked with: your device's clock is more than a day ahead. Set your clock right, then save your profile again.";

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

        // The links are a JSON object (2026-10-12): the copy for other servers is held to that
        // (`profile_copy::within_limits`), so a save that stored anything else (any JSON was
        // accepted) could never be signed, and the copy already out there, perhaps with a field
        // since kept for friends, would stay. Both apps always send an object.
        let links_object = matches!(serde_json::from_str::<serde_json::Value>(&socials), Ok(serde_json::Value::Object(_)));
        if socials.len() > 1024 || !links_object {
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

        // No line break in a one-line field (BUG-191): the words a copy's signature covers are one
        // field a line, and only the bio may hold a line break, so they split one way only
        // (`profile_copy::within_limits`, which every receiving server holds a copy to). No app's
        // inputs can type one; the trim above already took any at the ends.
        if [&pronoun, &loc, &avatar, &banner, &w_site, &socials].iter().any(|s| s.contains('\n')) {
            let _ = state.broadcast_tx.send(RelayMessage::Private {
                to: my_key.to_string(),
                message: "Pronouns, location and links cannot hold a line break.".to_string(),
            });
            return;
        }

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

        // The privacy map is an object of words (BUG-191): every side reads it one way
        // (`profile_copy::kept_for_friends`), and a map holding anything but words is one no app
        // writes. Any JSON at all was accepted until 2026-10-12.
        let words_only = matches!(
            serde_json::from_str::<serde_json::Value>(&priv_map),
            Ok(serde_json::Value::Object(map)) if map.values().all(serde_json::Value::is_string)
        );
        if !words_only {
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
                //
                // BUG-191: each copy names the person as THAT device's socket does (the name its
                // app signs into the copy for other servers), and asks it to sign that copy when
                // it changed (`ask_for_copy`, `just_saved`), so every online device re-signs and
                // the person's keys agree. Going unlisted asks for the withdrawal the same way. A
                // key not registered to the name (a placeholder name such as "Player", whose
                // profile row anyone using it shares) is never asked.
                let registered = state.db.keys_for_name(name).unwrap_or_default();
                let mut own_keys = registered.clone();
                if !own_keys.iter().any(|k| k == my_key) {
                    own_keys.push(my_key.to_string());
                }
                let socket_names: HashMap<String, String> = {
                    let peers = state.peers.read().await;
                    own_keys
                        .iter()
                        .filter_map(|k| peers.get(k).and_then(|p| p.display_name.clone()).map(|n| (k.clone(), n)))
                        .collect()
                };
                for key in own_keys {
                    let shown = socket_names.get(&key).cloned().unwrap_or_else(|| name.clone());
                    let ask = if registered.contains(&key) {
                        let copy = public_copy(&shown, &clean_bio, &avatar, &banner, &socials, &pronoun, &loc, &w_site, &priv_map);
                        ask_for_copy(state, &key, &copy, &priv_map, true)
                    } else {
                        CopyAsk::default()
                    };
                    let _ = state.broadcast_tx.send(RelayMessage::ProfileData {
                        name: shown,
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
                        signed_at: Some(ask.signed_at),
                        sign_now: Some(ask.sign_now),
                        target: Some(key),
                    });
                }
                // The save writes no copy for other servers itself (BUG-191): the devices asked
                // above sign it, and `handle_profile_sign` keeps it and passes it on.
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
    // map included, as on connect; an empty one when they have no profile here yet. It names them
    // as their socket does, not as they asked ("ann" for "Ann"), because their app signs that
    // name into the copy for other servers (BUG-191); and, as on connect, it asks them to sign
    // only when the copy out there shows more than they allow here now (`just_saved` false).
    if state.db.keys_for_name(&name).map(|keys| keys.iter().any(|k| k == my_key)).unwrap_or(false) {
        let own_name = state.peers.read().await.get(my_key).and_then(|p| p.display_name.clone()).unwrap_or_else(|| name.clone());
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
        let socials = if socials.trim().is_empty() { "{}".to_string() } else { socials };
        let privacy = if privacy.trim().is_empty() { "{}".to_string() } else { privacy };
        let copy = public_copy(&own_name, &bio, &avatar_url, &banner_url, &socials, &pronouns, &location, &website, &privacy);
        let ask = ask_for_copy(state, my_key, &copy, &privacy, false);
        let opt = |s: String| if s.is_empty() { None } else { Some(s) };
        let _ = state.broadcast_tx.send(RelayMessage::ProfileData {
            name: own_name,
            bio,
            socials,
            avatar_url: opt(avatar_url),
            banner_url: opt(banner_url),
            pronouns: opt(pronouns),
            location: opt(location),
            website: opt(website),
            accepts_files: Some(state.db.accepts_files_of(&name)),
            privacy: Some(privacy),
            stored: Some(stored),
            signed_at: Some(ask.signed_at),
            sign_now: Some(ask.sign_now),
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
/// stored, under `privacy`), as `profile_copy::sign_wanted` decides against the copy held here
/// (from any server); the ask is recorded in `RelayState::profile_sign_asks`, and never made for
/// a key that is not a person's (bots: `profile_copy::is_key_text`). `just_saved`: the echo of a
/// save (ask whenever the copy changed), rather than a connect or the person's own request (ask
/// only to narrow what is out there, so a device that connects never pushes an older profile over
/// a newer save made on another server). `signed_at` is the held copy's time, 0 for none.
pub fn ask_for_copy(state: &RelayState, key: &str, now: &PublicCopy, privacy: &str, just_saved: bool) -> CopyAsk {
    let held = match state.db.get_signed_profile(key) {
        Ok(held) => held,
        Err(e) => {
            tracing::error!("Profile: could not read the copy for other servers of {}: {e}", key_in_log(key));
            return CopyAsk::default();
        }
    };
    let signed_at = held.as_ref().map_or(0, |h| h.timestamp);
    let sign_now = profile_copy::is_key_text(key)
        && profile_copy::sign_wanted(held.map(|h| h.copy()).as_ref(), now, just_saved, privacy);
    if sign_now {
        let mut asks = state.profile_sign_asks.lock().unwrap_or_else(|p| p.into_inner());
        // Asks older than their lifetime are of no use: dropped here, so the map holds only the
        // keys asked in the last minute.
        asks.retain(|_, asked| asked.elapsed() < ASK_LIFETIME);
        asks.insert(key.to_string(), Instant::now());
    }
    CopyAsk { signed_at, sign_now }
}

/// Use up the ask for `key` (a copy was kept, or found not newer than the one held).
fn take_ask(state: &RelayState, key: &str) {
    state.profile_sign_asks.lock().unwrap_or_else(|p| p.into_inner()).remove(key);
}

/// A device's signature over its profile's copy for other servers (`profile_sign`, plan 3.5). The
/// frame carries no profile: the relay rebuilds the copy from what it stored under the socket's
/// own name, checks the signature, keeps the copy and passes it on. Cheapest checks first:
///
/// 1. an ask for this key in the last minute, else dropped (looked at, not taken: a copy that does
///    not match leaves it, because a save from another device may have changed the profile in
///    between, and that save's own copy is coming);
/// 2. a signature of the right shape; 3. the socket's name, registered to this key;
/// 4. the profile stored under it; 5. its public copy within the limits;
/// 6. dated at most a day ahead, else the person is told (the one refusal they can fix);
/// 7. newer than the copy held, else dropped and the ask used up;
/// 8. the signature checks, off the async pool (a panic counts as no), else dropped with no
///    notice: the app signs on its own, so the cause is a race or a broken app, which the shared
///    fixture catches, and a notice nobody can act on is noise;
/// 9. kept, the ask used up, and passed on to every linked server.
pub async fn handle_profile_sign(state: &Arc<RelayState>, my_key: &str, at: u64, signature: String) {
    let who = key_in_log(my_key);
    let asked = state
        .profile_sign_asks
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(my_key)
        .is_some_and(|asked| asked.elapsed() < ASK_LIFETIME);
    if !asked {
        tracing::debug!("Profile: dropped a copy for other servers from {who}: nobody asked for one");
        return;
    }
    if !profile_copy::is_signature_text(&signature) {
        tracing::debug!("Profile: dropped a copy for other servers from {who}: not a signature");
        return;
    }
    let Some(name) = state.peers.read().await.get(my_key).and_then(|p| p.display_name.clone()) else {
        return;
    };
    if !state.db.keys_for_name(&name).map(|keys| keys.iter().any(|k| k == my_key)).unwrap_or(false) {
        return;
    }
    let (bio, socials, avatar, banner, pronouns, location, website, privacy) = match state.db.get_profile_extended(&name) {
        Ok(Some(row)) => row,
        Ok(None) => return,
        Err(e) => {
            tracing::error!("Profile: could not read the profile of {who} to check its copy: {e}");
            return;
        }
    };
    let copy = public_copy(&name, &bio, &avatar, &banner, &socials, &pronouns, &location, &website, &privacy);
    if !profile_copy::within_limits(my_key, &copy) {
        return;
    }
    if at > crate::relay::storage::now_millis().saturating_add(MAX_AHEAD_MS) {
        let _ = state.broadcast_tx.send(RelayMessage::Private { to: my_key.to_string(), message: CLOCK_AHEAD_NOTICE.to_string() });
        return;
    }
    match state.db.get_signed_profile(my_key) {
        Ok(Some(held)) if held.timestamp >= at => {
            take_ask(state, my_key);
            tracing::debug!("Profile: dropped a copy for other servers from {who}: not newer than the one held");
            return;
        }
        Ok(_) => {}
        Err(e) => {
            tracing::error!("Profile: could not read the copy for other servers of {who}: {e}");
            return;
        }
    }
    let (key, checked, sig) = (my_key.to_string(), copy.clone(), signature.clone());
    let good = tokio::task::spawn_blocking(move || profile_copy::verify_copy(&key, &checked, at, &sig)).await.unwrap_or(false);
    if !good {
        tracing::info!("Profile: the copy for other servers from {who} does not match what this server stored; kept the one held");
        return;
    }
    match state.db.store_signed_profile(my_key, &copy, at, &signature) {
        Ok(kept) => {
            take_ask(state, my_key);
            if kept {
                gossip_profile(state, my_key, &copy, at, &signature, None).await;
                let withdrawn = if profile_copy::is_withdrawal(&copy) { " (withdrawn)" } else { "" };
                tracing::info!("Profile: kept and passed on the copy for other servers of {who}{withdrawn}");
            }
        }
        Err(e) => tracing::error!("Profile: could not keep the copy for other servers of {who}: {e}"),
    }
}

// ── An erase takes the copy off the linked servers (BUG-191 Wave 2B) ──

/// A person's signed withdrawal of the copy of their profile that other servers keep, sent with
/// their own erase (`account_delete`): the signature over `profile_copy::copy_text(key,
/// &withdrawal(), at)`, made by their own app (an erase leaves nothing to rebuild a copy from, and
/// the withdrawal is the same words for everyone, so the frame carries just these two). Only the
/// person's key can make one, so an admin's erase of someone else (`admin_erase`) has none: their
/// copy stays on the linked servers until they withdraw it themselves.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Withdrawal {
    /// Unix milliseconds, after the copy this server holds (`profile_copy::next_copy_time`).
    pub at: u64,
    /// Dilithium3 over the withdrawal's words, 6,618 lowercase hex characters.
    pub signature: String,
}

/// Before the person's own erase (msg_handlers.rs `handle_account_delete`, once its checks have
/// passed and BEFORE the account's rows go): keep their signed withdrawal over the copy held here
/// and pass it on to every linked server, which replaces the copy it holds, so erasing an account
/// also takes its profile off the servers this one is linked with (plan section 9, decision 2).
/// After the erase it would be too late: the copy held here is gone with the account, and a
/// withdrawal over nothing held is kept nowhere and goes nowhere.
///
/// The erase is the ask, so no `sign_now` is needed. Passed on only when it is the person's own
/// signature over the withdrawal and `store_signed_profile` keeps it: a copy is held here, the
/// withdrawal is newer than it and at most a day ahead. Nothing held here means nothing was passed
/// on from here. Anything else (none sent, an older app; one that does not check) passes nothing
/// on and never stops the erase. Returns whether it was passed on.
///
/// Its log lines name no key, as the erase's own do (sign_ups.rs `sign_up_logs_never_name_the_key`
/// reads this function): the log outlives what the person was promised.
pub async fn withdraw_on_erase(state: &Arc<RelayState>, my_key: &str, withdrawal: Option<Withdrawal>) -> bool {
    let Some(Withdrawal { at, signature }) = withdrawal else {
        return false;
    };
    if !profile_copy::is_key_text(my_key) || !profile_copy::is_signature_text(&signature) {
        return false;
    }
    let (key, sig) = (my_key.to_string(), signature.clone());
    let good = tokio::task::spawn_blocking(move || profile_copy::verify_copy(&key, &profile_copy::withdrawal(), at, &sig))
        .await
        .unwrap_or(false);
    if !good {
        tracing::info!("Profile: an erase came with a withdrawal that is not the person's signature over one; nothing passed on");
        return false;
    }
    let copy = profile_copy::withdrawal();
    match state.db.store_signed_profile(my_key, &copy, at, &signature) {
        Ok(true) => {
            gossip_profile(state, my_key, &copy, at, &signature, None).await;
            tracing::info!("Profile: an erase passed on its withdrawal of the copy for other servers");
            true
        }
        Ok(false) => {
            tracing::debug!("Profile: an erase's withdrawal was not passed on: no copy held here, or not newer than it");
            false
        }
        Err(e) => {
            tracing::error!("Profile: could not keep an erase's withdrawal: {e}");
            false
        }
    }
}

/// The profile handlers' tests: the field `accepts_files` (2026-10-11, docs/design/media-showcase.md),
/// the person's own copy, and what the lookups show.
#[cfg(test)]
#[path = "profile_tests.rs"]
mod profile_tests;
