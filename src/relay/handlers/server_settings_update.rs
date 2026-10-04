//! Server settings, the admin update (v0.200.0): `server_settings_update` from Server
//! Settings > ADMIN. Each field is optional; a missing field keeps its current value.
//! Admin-only; on success the expiry pass runs at once (a lowered erase window or cap
//! holds right away, storage/expiry.rs) and the new `server_settings_state` goes to
//! every client.
//! The shared world's clock speed (`world_time_scale`) also reaches the running
//! world and every connected game from here, with no restart (`set_world_clock`).
//!
//! Its own file because relay.rs is held to a line budget (tests/file_size_ratchet.rs).

use crate::relay::relay::{RelayMessage, RelayState};
use std::sync::Arc;

/// Run the shared world's clock at `scale` game seconds per real second from
/// now on, and tell every connected game at once with a `game_time_sync`, so
/// nobody keeps the old pace until the next 5-second word (relay/mod.rs).
/// The clock keeps its date; only how fast it runs changes. Nothing is sent
/// when the speed is unchanged, or on a server with the game switched off.
pub async fn set_world_clock(state: &Arc<RelayState>, scale: f64) {
    let sync = {
        let mut world = state.game_world.write().await;
        if world.time_scale == scale {
            return;
        }
        world.time_scale = scale;
        world.time_sync_json()
    };
    tracing::info!("Shared world clock now runs at {scale}x");
    if state.features.enabled(crate::relay::features::Feature::Game) {
        let _ = state.broadcast_tx.send(RelayMessage::System { message: format!("__game__:{sync}") });
    }
}

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
        erased_accounts_ttl_days, erased_accounts_cap,
        world_time_scale,
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
        // Erased accounts remembered (BUG-135): held to the ranges the page offers.
        use crate::relay::storage::{ERASED_ACCOUNTS_CAP_RANGE as CAP, ERASED_ACCOUNTS_TTL_DAYS_RANGE as TTL};
        if let Some(v) = erased_accounts_ttl_days { current.erased_accounts_ttl_days = v.clamp(TTL.0, TTL.1); }
        if let Some(v) = erased_accounts_cap { current.erased_accounts_cap = v.clamp(CAP.0, CAP.1); }
        // The shared world's clock speed (2026-10-04), held to the player
        // Time setting's 1..=1000; a value that is not a number changes nothing.
        if let Some(v) = world_time_scale.and_then(crate::relay::storage::clamp_world_time_scale) {
            current.world_time_scale = v;
        }
        match state_clone.db.set_server_settings(&current, &my_key_for_recv) {
            Ok(true) => {
                state_clone.db.run_expiry_sweeps(); // a lowered window or cap holds at once (storage/expiry.rs)
                // Saved first, then the running world: a restart keeps it.
                set_world_clock(state, current.world_time_scale).await;
                // Broadcast new state to everyone (no target: an admin's change).
                let _ = state_clone.broadcast_tx.send(
                    RelayMessage::ServerSettingsState { settings: current, target: None }
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

#[cfg(test)]
mod tests {
    use crate::relay::relay::RelayState;
    use crate::relay::storage::Storage;

    /// A RESTART KEEPS THE ADMIN'S SPEED (review of 2026-10-04, finding 1).
    /// An admin sets the shared world to 24x, the relay restarts, and the
    /// world must come back at 24x, not at the 72x a new world starts with.
    /// The other tests could not catch this: every test database is new, so
    /// it already says 72, the same as `GameWorld::new()`.
    ///
    /// Seen red 2026-10-04 with the line in `RelayState::new` that reads the
    /// saved speed removed: "after a restart the world runs at the saved 24x,
    /// left: 72.0, right: 24.0".
    #[test]
    fn a_restart_brings_the_shared_world_back_at_the_saved_speed() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir()
            .join(format!("hum_world_clock_restart_{}_{nanos}.db", std::process::id()));
        {
            // The relay before the restart: the admin's Apply saved 24x.
            let db = Storage::open(&path).expect("open test db");
            let mut s = db.get_server_settings().expect("settings row");
            assert_eq!(s.world_time_scale, 72.0, "a new server starts at 72x");
            s.world_time_scale = 24.0;
            assert!(db.set_server_settings(&s, "test_admin").expect("save"), "the row was updated");
        }
        // The relay after the restart: the same file, a new RelayState.
        let state = RelayState::new(Storage::open(&path).expect("reopen test db"));
        let scale = state.game_world.try_read().expect("nothing else holds the world").time_scale;
        assert_eq!(scale, 24.0, "after a restart the world runs at the saved 24x");
        drop(state);
        let _ = std::fs::remove_file(&path);
    }
}
