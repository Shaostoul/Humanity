//! Keeping other servers' backups (2026-10-22): the receiving side of `peer.rs`.
//!
//! An admin lists the servers this one keeps backups for (Backups panel, "Keep other servers'
//! backups here"): each by its public key, with a label and a size limit. Their copies arrive
//! already sealed with THEIR backup key, so this server stores ciphertext it cannot read, in
//! `<data>/backup-host/<their fingerprint>/`, one file per object, written whole or not at all
//! (`dest::FolderDest`). Every request must carry a fresh Dilithium3 signature by the listed
//! key over the operation, the object's name, the body's hash and the time; anything else is
//! refused before a byte is stored. A server that is not listed gets 403 and learns nothing.
//!
//! The list is `<data>/backup-host.json`: public keys and limits only, nothing secret.

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use super::dest::{check_name, FolderDest};
use super::peer::{fingerprint, preimage, H_FROM, H_SIG, H_TIME};
use crate::relay::relay::RelayState;

/// The largest object a sending server may store in one request (a snapshot chunk or a
/// segment is at most `CHUNK` plus a little; nginx's own limit on /api/ is 6 MB).
pub const MAX_OBJECT: usize = 6 * 1024 * 1024;
/// The body limit on the upload route: the object plus nothing.
pub const BODY_LIMIT: usize = MAX_OBJECT;

/// One server whose backups are kept here.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Sender {
    pub label: String,
    /// Its Dilithium3 public key, hex (3,904 characters).
    pub public_key: String,
    pub fingerprint: String,
    pub quota_bytes: u64,
    pub added_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostConfig {
    #[serde(default)]
    pub senders: Vec<Sender>,
}

pub fn config_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("backup-host.json")
}

pub fn store_dir(data_dir: &std::path::Path, fp: &str) -> PathBuf {
    data_dir.join("backup-host").join(fp)
}

pub fn load(data_dir: &std::path::Path) -> HostConfig {
    std::fs::read_to_string(config_path(data_dir)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn save(data_dir: &std::path::Path, cfg: &HostConfig) -> Result<(), String> {
    let text = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    super::write_file_atomic(&config_path(data_dir), text.as_bytes(), false)
}

/// Add (or update the label and limit of) a server by its public key.
pub fn allow(data_dir: &std::path::Path, label: &str, public_key: &str, quota_bytes: u64, now_ms: u64) -> Result<String, String> {
    let key = public_key.trim().to_lowercase();
    if !crate::relay::handlers::federation::is_server_key(&key) {
        return Err(format!(
            "A server's key is {} hex characters: copy it from that server's Backups panel.",
            crate::relay::handlers::federation::SERVER_KEY_HEX_LEN
        ));
    }
    let raw = hex::decode(&key).map_err(|_| "That key is not hex.".to_string())?;
    let fp = fingerprint(&raw);
    let mut cfg = load(data_dir);
    let label = label.trim().chars().take(80).collect::<String>();
    let label = if label.is_empty() { format!("server {}", &fp[..8]) } else { label };
    match cfg.senders.iter_mut().find(|s| s.fingerprint == fp) {
        Some(s) => {
            s.label = label.clone();
            s.quota_bytes = quota_bytes;
        }
        None => cfg.senders.push(Sender { label: label.clone(), public_key: key, fingerprint: fp.clone(), quota_bytes, added_ms: now_ms }),
    }
    save(data_dir, &cfg)?;
    Ok(format!("This server now keeps up to {} of backups for {label}.", super::human_bytes(quota_bytes)))
}

/// Stop keeping a server's backups. Its copies are deleted only when `delete_copies` is set.
pub fn remove(data_dir: &std::path::Path, fp: &str, delete_copies: bool) -> Result<String, String> {
    let mut cfg = load(data_dir);
    let before = cfg.senders.len();
    let label = cfg.senders.iter().find(|s| s.fingerprint == fp).map(|s| s.label.clone()).unwrap_or_default();
    cfg.senders.retain(|s| s.fingerprint != fp);
    if cfg.senders.len() == before {
        return Err("That server is not on the list.".into());
    }
    save(data_dir, &cfg)?;
    if delete_copies && fp.len() == 32 && fp.bytes().all(|b| b.is_ascii_hexdigit()) {
        let _ = std::fs::remove_dir_all(store_dir(data_dir, fp));
        Ok(format!("Stopped keeping backups for {label}, and deleted the copies kept here."))
    } else {
        Ok(format!("Stopped keeping backups for {label}. The copies already here stay until you delete them."))
    }
}

/// Bytes and objects kept for one server.
pub fn usage(data_dir: &std::path::Path, fp: &str) -> (u64, u64) {
    fn walk(d: &std::path::Path, acc: &mut (u64, u64)) {
        if let Ok(rd) = std::fs::read_dir(d) {
            for e in rd.flatten() {
                match e.file_type() {
                    Ok(t) if t.is_dir() => walk(&e.path(), acc),
                    Ok(t) if t.is_file() => {
                        acc.0 += e.metadata().map(|m| m.len()).unwrap_or(0);
                        acc.1 += 1;
                    }
                    _ => {}
                }
            }
        }
    }
    let mut acc = (0, 0);
    walk(&store_dir(data_dir, fp), &mut acc);
    acc
}

/// What the Backups panel shows of this side.
pub fn view(data_dir: &std::path::Path) -> serde_json::Value {
    let cfg = load(data_dir);
    let senders: Vec<serde_json::Value> = cfg
        .senders
        .iter()
        .map(|s| {
            let (used, objects) = usage(data_dir, &s.fingerprint);
            serde_json::json!({
                "label": s.label,
                "fingerprint": s.fingerprint,
                "quota_bytes": s.quota_bytes,
                "used_bytes": used,
                "objects": objects,
                "added_ms": s.added_ms,
            })
        })
        .collect();
    serde_json::json!({ "senders": senders })
}

/// Another server's copies are refused once this server's own disk is this full, whatever its
/// limit says: keeping someone else's backups must never fill the disk this server's own
/// database lives on (the VPS disk filled once already, BUG-169).
pub const FULL_PCT: u32 = 90;

/// Whether the disk holding `dir` is FULL_PCT full or more, asked of `df` at most once a minute.
/// False where `df` is not there to ask (a Windows desktop node): its limit per server still holds.
fn disk_nearly_full(dir: &std::path::Path) -> bool {
    static LAST: std::sync::Mutex<Option<(std::time::Instant, bool)>> = std::sync::Mutex::new(None);
    let mut last = LAST.lock().unwrap();
    if let Some((at, full)) = *last {
        if at.elapsed() < std::time::Duration::from_secs(60) {
            return full;
        }
    }
    let full = std::process::Command::new("df")
        .args(["-P", "-k"])
        .arg(dir)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            let text = String::from_utf8_lossy(&o.stdout).to_string();
            let row = text.lines().nth(1)?.to_string();
            row.split_whitespace().nth(4)?.trim_end_matches('%').parse::<u32>().ok()
        })
        .is_some_and(|pct| pct >= FULL_PCT);
    *last = Some((std::time::Instant::now(), full));
    full
}

fn refuse(status: StatusCode, why: &str) -> Response {
    (status, why.to_string()).into_response()
}

/// Who sent this request, if it is a listed server's fresh signature over exactly this
/// operation, name and body.
async fn authorize(state: &RelayState, headers: &HeaderMap, op: &str, name: &str, body: &[u8]) -> Result<(PathBuf, Sender), Response> {
    let data_dir = super::live_data_dir(&state.db);
    let h = |k: &str| headers.get(k).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let (from, sig) = (h(H_FROM), h(H_SIG));
    let ms: u64 = h(H_TIME).parse().unwrap_or(0);
    if from.is_empty() || sig.is_empty() || ms == 0 {
        return Err(refuse(StatusCode::UNAUTHORIZED, "A signed request from a server is needed."));
    }
    let Some(sender) = load(&data_dir).senders.into_iter().find(|s| s.fingerprint == from) else {
        return Err(refuse(StatusCode::FORBIDDEN, "This server does not keep backups for that server."));
    };
    let now = crate::relay::storage::now_millis();
    if now.saturating_sub(ms) > 5 * 60 * 1000 || ms > now + 60_000 {
        return Err(refuse(StatusCode::UNAUTHORIZED, "The request's time is more than five minutes off: check the clock."));
    }
    let (pk, words) = (sender.public_key.clone(), preimage(op, name, body, ms));
    let ok = tokio::task::spawn_blocking(move || {
        let (Ok(pk), Ok(sig)) = (hex::decode(&pk), hex::decode(&sig)) else { return false };
        crate::relay::core::pq_crypto::verify_dilithium(&pk, words.as_bytes(), &sig).is_ok()
    })
    .await
    .unwrap_or(false);
    if !ok {
        return Err(refuse(StatusCode::UNAUTHORIZED, "The signature does not match this request."));
    }
    if !state.auth_nonce_fresh(&sender.public_key, &format!("backup_host:{op}:{name}"), ms) {
        return Err(refuse(StatusCode::UNAUTHORIZED, "That request was already used."));
    }
    Ok((data_dir, sender))
}

/// PUT /api/backup-host/o/{*name}
pub async fn put_object(State(state): State<Arc<RelayState>>, Path(name): Path<String>, headers: HeaderMap, body: Bytes) -> Response {
    if check_name(&name).is_err() {
        return refuse(StatusCode::BAD_REQUEST, "Not a backup object name.");
    }
    if body.len() > MAX_OBJECT {
        return refuse(StatusCode::PAYLOAD_TOO_LARGE, "Too big for one object.");
    }
    let (dir, sender) = match authorize(&state, &headers, "put", &name, &body).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    if disk_nearly_full(&dir) {
        tracing::warn!("backup host: refused a copy for {}: this server's disk is {}% full or more", sender.label, FULL_PCT);
        return refuse(StatusCode::INSUFFICIENT_STORAGE, "This server's own disk is nearly full.");
    }
    let store = store_dir(&dir, &sender.fingerprint);
    let existing = std::fs::metadata(name.split('/').fold(store.clone(), |p, c| p.join(c))).map(|m| m.len()).unwrap_or(0);
    let (used, _) = usage(&dir, &sender.fingerprint);
    if used.saturating_sub(existing) + body.len() as u64 > sender.quota_bytes {
        return refuse(StatusCode::INSUFFICIENT_STORAGE, "Over the limit this server keeps for you.");
    }
    match FolderDest::new(store).put(&name, body.to_vec()).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            tracing::error!("backup host: storing an object for {} failed: {e}", sender.label);
            refuse(StatusCode::INTERNAL_SERVER_ERROR, "Could not store it.")
        }
    }
}

/// GET /api/backup-host/o/{*name}
pub async fn get_object(State(state): State<Arc<RelayState>>, Path(name): Path<String>, headers: HeaderMap) -> Response {
    if check_name(&name).is_err() {
        return refuse(StatusCode::BAD_REQUEST, "Not a backup object name.");
    }
    let (dir, sender) = match authorize(&state, &headers, "get", &name, &[]).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    match FolderDest::new(store_dir(&dir, &sender.fingerprint)).get(&name).await {
        Ok(Some(b)) => (StatusCode::OK, b).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => refuse(StatusCode::INTERNAL_SERVER_ERROR, "Could not read it."),
    }
}

/// DELETE /api/backup-host/o/{*name}
pub async fn delete_object(State(state): State<Arc<RelayState>>, Path(name): Path<String>, headers: HeaderMap) -> Response {
    if check_name(&name).is_err() {
        return refuse(StatusCode::BAD_REQUEST, "Not a backup object name.");
    }
    let (dir, sender) = match authorize(&state, &headers, "delete", &name, &[]).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    match FolderDest::new(store_dir(&dir, &sender.fingerprint)).delete(&name).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => refuse(StatusCode::INTERNAL_SERVER_ERROR, "Could not delete it."),
    }
}

/// GET /api/backup-host/list
pub async fn list_objects(State(state): State<Arc<RelayState>>, headers: HeaderMap) -> Response {
    let (dir, sender) = match authorize(&state, &headers, "list", "", &[]).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    match FolderDest::new(store_dir(&dir, &sender.fingerprint)).list().await {
        Ok(items) => {
            let used: u64 = items.iter().map(|i| i.size).sum();
            let objects: Vec<serde_json::Value> = items.iter().map(|i| serde_json::json!({ "name": i.name, "size": i.size })).collect();
            axum::Json(serde_json::json!({ "objects": objects, "used_bytes": used, "quota_bytes": sender.quota_bytes })).into_response()
        }
        Err(_) => refuse(StatusCode::INTERNAL_SERVER_ERROR, "Could not list them."),
    }
}
