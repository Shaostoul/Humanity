//! Server settings, the admin update (v0.200.0): `server_settings_update` from Server
//! Settings > ADMIN. Each field is optional; a missing field keeps its current value.
//! Admin-only; on success the new `server_settings_state` goes to every client.
//!
//! Its own file because relay.rs is held to a line budget (tests/file_size_ratchet.rs).

use crate::relay::relay::{RelayMessage, RelayState};
use std::sync::Arc;

/// Apply one `server_settings_update` from `my_key` (the relay checks the role here).
pub async fn handle(state: &Arc<RelayState>, my_key: &str, upd: RelayMessage) {
    let state_clone = state;
    let my_key_for_recv = my_key.to_string();
    let RelayMessage::ServerSettingsUpdate {
        max_chars_unverified, max_chars_verified, max_chars_mod, max_chars_admin,
        image_sharing_enabled, file_sharing_enabled, max_upload_mb,
        max_upload_mb_unverified, max_upload_mb_verified,
        max_upload_mb_mod, max_upload_mb_admin,
        voice_channels_enabled, video_streaming_enabled, allowed_file_extensions,
        max_uploads_per_user, max_total_upload_mb,
        max_uploads_per_user_unverified, max_uploads_per_user_verified,
        max_uploads_per_user_mod, max_uploads_per_user_admin,
        require_pq_signatures,
        p2p_distribution_enabled,
        server_description,
        server_name,
        local_channel_enabled,
        dm_mailbox_ttl_days,
        message_retention_days,
    } = upd else { return };
    let role = state_clone.db.get_role(&my_key_for_recv).unwrap_or_default();
    if role != "admin" && role != "owner" {
        let private = RelayMessage::Private {
            to: my_key_for_recv.clone(),
            message: "Only admins can update server settings.".to_string(),
        };
        let _ = state_clone.broadcast_tx.send(private);
    } else {
        let mut current = state_clone.db.get_server_settings().unwrap_or_default();
        // Defensive bounds — stop the operator from typing
        // a negative or absurdly large char limit by accident.
        let clamp = |v: i64| v.clamp(1, 1_000_000);
        let clamp_mb = |v: i64| v.clamp(1, 10_000);
        if let Some(v) = max_chars_unverified  { current.max_chars_unverified  = clamp(v); }
        if let Some(v) = max_chars_verified    { current.max_chars_verified    = clamp(v); }
        if let Some(v) = max_chars_mod         { current.max_chars_mod         = clamp(v); }
        if let Some(v) = max_chars_admin       { current.max_chars_admin       = clamp(v); }
        if let Some(v) = image_sharing_enabled { current.image_sharing_enabled = v; }
        if let Some(v) = file_sharing_enabled  { current.file_sharing_enabled  = v; }
        // v0.200 legacy single max_upload_mb — if a v0.200
        // client sends only this field, propagate it to all
        // four per-role columns so behavior matches what
        // the operator presumably intended.
        if let Some(v) = max_upload_mb {
            let v = clamp_mb(v);
            current.max_upload_mb_unverified = v;
            current.max_upload_mb_verified   = v;
            current.max_upload_mb_mod        = v;
            current.max_upload_mb_admin      = v;
        }
        // v0.201 per-role upload caps. Override anything
        // the legacy field above set if both were sent.
        if let Some(v) = max_upload_mb_unverified { current.max_upload_mb_unverified = clamp_mb(v); }
        if let Some(v) = max_upload_mb_verified   { current.max_upload_mb_verified   = clamp_mb(v); }
        if let Some(v) = max_upload_mb_mod        { current.max_upload_mb_mod        = clamp_mb(v); }
        if let Some(v) = max_upload_mb_admin      { current.max_upload_mb_admin      = clamp_mb(v); }
        if let Some(v) = voice_channels_enabled  { current.voice_channels_enabled  = v; }
        if let Some(v) = video_streaming_enabled { current.video_streaming_enabled = v; }
        if let Some(v) = allowed_file_extensions {
            // Light hygiene — lowercase, strip whitespace, drop empties.
            let cleaned: Vec<String> = v.split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            current.allowed_file_extensions = cleaned.join(",");
        }
        // v0.237 upload-storage limits. Per-user
        // FIFO clamped 1..=1000; total disk cap
        // clamped 1..=1_000_000 MB (1 TB ceiling).
        // v0.237 legacy single FIFO — if an old
        // client sends only this, fan it out to
        // all four per-role columns.
        if let Some(v) = max_uploads_per_user {
            let v = v.clamp(1, 1_000);
            current.max_uploads_per_user_unverified = v;
            current.max_uploads_per_user_verified   = v;
            current.max_uploads_per_user_mod         = v;
            current.max_uploads_per_user_admin       = v;
        }
        if let Some(v) = max_total_upload_mb {
            current.max_total_upload_mb = v.clamp(1, 1_000_000);
        }
        // v0.238 per-role FIFO retention. Override
        // the legacy fan-out if both were sent.
        if let Some(v) = max_uploads_per_user_unverified { current.max_uploads_per_user_unverified = v.clamp(1, 1_000); }
        if let Some(v) = max_uploads_per_user_verified   { current.max_uploads_per_user_verified   = v.clamp(1, 1_000); }
        if let Some(v) = max_uploads_per_user_mod        { current.max_uploads_per_user_mod        = v.clamp(1, 1_000); }
        if let Some(v) = max_uploads_per_user_admin      { current.max_uploads_per_user_admin      = v.clamp(1, 1_000); }
        // PQ Inc 3: hard-enforcement toggle.
        if let Some(v) = require_pq_signatures { current.require_pq_signatures = v; }
        // Server→Services: P2P soft gate.
        if let Some(v) = p2p_distribution_enabled { current.p2p_distribution_enabled = v; }
        // Operator-set server description (launcher detail pane, v0.478).
        // Bounded so a huge paste can't bloat the row.
        if let Some(v) = server_description { current.server_description = v.chars().take(2000).collect(); }
        // Operator-set server name (v0.480). Tighter cap than the description.
        if let Some(v) = server_name { current.server_name = v.chars().take(120).collect(); }
        if let Some(v) = local_channel_enabled {
            let turning_on = v && !current.local_channel_enabled;
            current.local_channel_enabled = v;
            // Re-enabling the guarantee reseeds #local right
            // away if none is left (the boot-time seeding
            // would otherwise wait for a restart).
            if turning_on && !state_clone.db.any_local_only_channel().unwrap_or(false) {
                let _ = state_clone.db.create_channel(
                    "local",
                    "local",
                    Some("This server only. Never bridged to other servers."),
                    "system",
                    false,
                );
                let _ = state_clone.db.set_channel_local_only("local", true);
                let _ = state_clone.db.set_channel_position("local", 3);
                crate::relay::handlers::broadcast::broadcast_channel_list(&state_clone);
            }
        }
        // Sealed-sender DM mailbox TTL: clamped to a sane
        // window (1 day floor; a year ceiling — the point
        // of the mailbox is that it is NOT an archive).
        if let Some(v) = dm_mailbox_ttl_days {
            current.dm_mailbox_ttl_days = v.clamp(1, 365);
        }
        // Message retention: 0 = forever, else clamp to a year.
        if let Some(v) = message_retention_days {
            current.message_retention_days = if v <= 0 { 0 } else { v.min(3650) };
        }
        match state_clone.db.set_server_settings(&current, &my_key_for_recv) {
            Ok(true) => {
                // Broadcast new state to everyone.
                let _ = state_clone.broadcast_tx.send(
                    RelayMessage::ServerSettingsState { settings: current }
                );
                let sys = RelayMessage::System {
                    message: format!("Server settings updated by admin."),
                };
                let _ = state_clone.broadcast_tx.send(sys);
            }
            Ok(false) => {
                let private = RelayMessage::Private {
                    to: my_key_for_recv.clone(),
                    message: "Server settings row missing — relay restart needed.".to_string(),
                };
                let _ = state_clone.broadcast_tx.send(private);
            }
            Err(e) => {
                tracing::error!("server_settings_update failed: {e}");
                let private = RelayMessage::Private {
                    to: my_key_for_recv.clone(),
                    message: format!("Update failed: {e}"),
                };
                let _ = state_clone.broadcast_tx.send(private);
            }
        }
    }
}
