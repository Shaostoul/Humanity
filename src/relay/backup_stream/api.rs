//! `POST /api/admin/backups`: the one endpoint behind the Backups panel on both apps (native
//! Server Settings > ADMIN > Backups, and the web admin page), so the two cannot drift. One
//! action per request, each listed in `data/admin/ops_registry.json`:
//!
//! | action | what it does |
//! |---|---|
//! | `status` | everything the panel shows (no secret ever) |
//! | `back_up_now` | a sealed copy in `backups/`, and a fresh full copy to the destination |
//! | `restore_drill` | restore from the destination into a scratch file and compare (`mod.rs` `drill`) |
//! | `test_destination` | try a destination without saving it |
//! | `set_destination` | save where the copies go, how much history, the alert threshold (tested first) |
//! | `host_allow` / `host_remove` | which other servers may keep their backups here |
//!
//! Auth is the house pattern for admin REST: the admin's Dilithium3 signature, five minutes
//! fresh and used once, here over `admin_backups\n<action>\n<sha256 of params>` then
//! `\n<timestamp>`. Binding the hash of the exact `params` string means a signature for one
//! request cannot carry other settings, without either side having to agree on how JSON is
//! written. The params travel in the JSON body, never in the address.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::{host, live_data_dir, SetRequest};
use crate::relay::relay::RelayState;
use crate::relay::storage::Storage;

#[derive(Debug, Deserialize)]
pub struct Req {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub timestamp: u64,
    #[serde(default)]
    pub sig: String,
    pub action: String,
    /// The action's arguments as a JSON text (signed by its hash). Empty for none.
    #[serde(default)]
    pub params: String,
}

/// The words an admin's app signs for one request.
pub fn purpose(action: &str, params: &str) -> String {
    format!("admin_backups\n{action}\n{}", hex::encode(Sha256::digest(params.as_bytes())))
}

fn answer(status: StatusCode, ok: bool, message: &str, db: Option<&Storage>) -> Response {
    let mut body = serde_json::json!({ "ok": ok, "message": message });
    if let Some(db) = db {
        body["status"] = status_json(db);
    }
    (status, Json(body)).into_response()
}

/// Everything the Backups panel shows. No secret: the destination is shown through
/// `DestinationConfig::view`, the backup key only as its fingerprint, the sending key only as
/// its public half.
pub fn status_json(db: &Storage) -> serde_json::Value {
    let data = live_data_dir(db);
    let cfg = super::config_with_sender(&data);
    let mut stream = db.backup_stream_status();
    if !cfg.active() {
        stream.state = "off".into();
        stream.words = "Continuous backup is off. Choose where the copies go, below, and turn it on.".into();
    }
    if stream.key_fingerprint.is_empty() {
        if let Some(k) = crate::relay::storage::backup_crypto::load_or_create_key(&data) {
            stream.key_fingerprint = super::seal::key_fingerprint(&k);
        }
    }
    let sending_key = cfg.sending_key().unwrap_or_default();
    let sending_fp = hex::decode(&sending_key).map(|b| super::peer::fingerprint(&b)).unwrap_or_default();
    serde_json::json!({
        "stream": stream,
        "config": cfg.view(),
        "local_backups": crate::relay::storage::backups::list_backups(),
        "manual_kept": crate::relay::storage::backups::MANUAL_BACKUPS_KEPT,
        "host": host::view(&data),
        "sending_key": sending_key,
        "sending_fingerprint": sending_fp,
        "ranges": {
            "retention_hours": [super::RETENTION_RANGE.0, super::RETENTION_RANGE.1],
            "snapshot_every_hours": [super::SNAPSHOT_RANGE.0, super::SNAPSHOT_RANGE.1],
            "alert_after_minutes": [super::ALERT_RANGE.0, super::ALERT_RANGE.1],
        },
    })
}

#[derive(Deserialize)]
struct TestParams {
    destination: super::dest::DestinationConfig,
}

#[derive(Deserialize)]
struct AllowParams {
    #[serde(default)]
    label: String,
    public_key: String,
    /// The space given, in GB (decimal places allowed).
    quota_gb: f64,
}

#[derive(Deserialize)]
struct RemoveParams {
    fingerprint: String,
    #[serde(default)]
    delete_copies: bool,
}

/// POST /api/admin/backups
pub async fn post_admin_backups(State(state): State<Arc<RelayState>>, Json(req): Json<Req>) -> Response {
    use crate::relay::handlers::broadcast::verify_dilithium_signature;
    if req.key.is_empty() || req.sig.is_empty() {
        return answer(StatusCode::UNAUTHORIZED, false, "Only this server's admins can see or change its backups: sign in as an admin first.", None);
    }
    let now = crate::relay::storage::now_millis();
    if now.saturating_sub(req.timestamp) > 5 * 60 * 1000 || req.timestamp > now + 60_000 {
        return answer(StatusCode::BAD_REQUEST, false, "The request is too old or from the future: check this computer's clock.", None);
    }
    let (vk, vsig, words, ts) = (req.key.clone(), req.sig.clone(), purpose(&req.action, &req.params), req.timestamp);
    let ok = tokio::task::spawn_blocking(move || verify_dilithium_signature(&vk, &words, ts, &vsig)).await.unwrap_or(false);
    if !ok {
        return answer(StatusCode::UNAUTHORIZED, false, "The signature does not match this request.", None);
    }
    if !state.auth_nonce_fresh(&req.key, "admin_backups", req.timestamp) {
        return answer(StatusCode::UNAUTHORIZED, false, "That request was already used: try again.", None);
    }
    let role = state.db.get_role(&req.key).unwrap_or_default();
    if role != "admin" && role != "owner" {
        return answer(StatusCode::FORBIDDEN, false, "Only this server's admins can see or change its backups.", None);
    }
    let db = &state.db;
    let data = live_data_dir(db);
    let result: Result<String, String> = match req.action.as_str() {
        "status" => Ok(String::new()),
        "back_up_now" => {
            let s = state.clone();
            match tokio::task::spawn_blocking(move || s.db.backup_now()).await {
                Ok(Ok(done)) => {
                    tracing::info!("Back up now (admin page) by {}: {}", &req.key[..12.min(req.key.len())], done.entry.file);
                    Ok(done.reply())
                }
                Ok(Err(e)) => Err(format!("Backup FAILED: {e}")),
                Err(e) => Err(format!("Backup FAILED: {e}")),
            }
        }
        "restore_drill" => {
            if db.backup_stream_status().drill.running {
                Ok("A restore drill is already running.".into())
            } else if !db.stream.config().active() {
                Err("Continuous backup is off: there is nothing to drill a restore from.".into())
            } else {
                let s = state.clone();
                tokio::spawn(async move {
                    super::drill(s).await;
                });
                Ok("Restore drill started: it restores from the destination into a scratch file and compares it with the live database. The result appears here.".into())
            }
        }
        "test_destination" => match serde_json::from_str::<TestParams>(&req.params) {
            Ok(p) => {
                let cfg = super::config_with_sender(&data);
                let d = p.destination.merged_with_saved(cfg.destination.as_ref());
                super::test_destination(db, &d, cfg.sender_seed_bytes().as_ref()).await
            }
            Err(e) => Err(format!("Those settings are not readable: {e}")),
        },
        "set_destination" => match serde_json::from_str::<SetRequest>(&req.params) {
            Ok(p) => super::apply_settings(db, p).await,
            Err(e) => Err(format!("Those settings are not readable: {e}")),
        },
        "host_allow" => match serde_json::from_str::<AllowParams>(&req.params) {
            Ok(p) if p.quota_gb > 0.0 && p.quota_gb <= 10_000.0 => host::allow(&data, &p.label, &p.public_key, (p.quota_gb * 1_073_741_824.0) as u64, now),
            Ok(_) => Err("Give the space in GB, more than 0.".into()),
            Err(e) => Err(format!("Those settings are not readable: {e}")),
        },
        "host_remove" => match serde_json::from_str::<RemoveParams>(&req.params) {
            Ok(p) => host::remove(&data, &p.fingerprint, p.delete_copies),
            Err(e) => Err(format!("Those settings are not readable: {e}")),
        },
        other => Err(format!("Unknown action {other}.")),
    };
    match result {
        Ok(m) => answer(StatusCode::OK, true, &m, Some(db)),
        Err(m) => answer(StatusCode::BAD_REQUEST, false, &m, Some(db)),
    }
}

#[cfg(test)]
mod tests {
    /// The words the apps sign. The web panel's test (scripts/tests/backups-admin-web.test.js)
    /// asserts the same two strings, so a change on either side is caught on both; the desktop
    /// panel calls this function itself.
    #[test]
    fn the_signed_words_match_the_apps() {
        assert_eq!(super::purpose("status", ""), "admin_backups\nstatus\ne3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(
            super::purpose("set_destination", "{\"enabled\":false}"),
            "admin_backups\nset_destination\n5acf3ff77b4420677b5923071f303facaba7a9273a346284a667a275df325146"
        );
    }
}
