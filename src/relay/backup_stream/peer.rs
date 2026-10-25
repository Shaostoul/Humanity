//! Another HumanityOS server as a backup destination (2026-10-22): the sending side.
//!
//! Two servers that trust each other can keep each other's backups: no bucket to rent, no
//! account to make. The server keeping the copies lists this server's key under "Keep other
//! servers' backups here" (its Backups panel; `host.rs` is that side), with a size limit. This
//! side signs every request with this server's own Dilithium3 identity (the key federation
//! already uses), over the operation, the object's name, a hash of the body and the time, so
//! a request cannot be replayed, redirected to another object or changed on the way. What it
//! sends is sealed with this server's backup key before it leaves (`seal.rs`): the server
//! keeping it holds only ciphertext it cannot open.

use sha2::{Digest, Sha256};

use super::names::Listed;
use crate::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};

/// Request headers. The signature is about 6,600 hex characters: a header, never the address
/// (a URL that long is refused by nginx, the admin-stats lesson of v0.851).
pub const H_FROM: &str = "x-hos-backup-from";
pub const H_TIME: &str = "x-hos-backup-time";
pub const H_SIG: &str = "x-hos-backup-sig";

/// The words signed for one request.
pub fn preimage(op: &str, name: &str, body: &[u8], ms: u64) -> String {
    format!("hos-backup-host\n{op}\n{name}\n{}\n{ms}", hex::encode(Sha256::digest(body)))
}

/// A server's short fingerprint: the first 16 bytes of BLAKE3 over its public key, in hex.
/// How a backup host names the folder it keeps a server's copies in.
pub fn fingerprint(public_key: &[u8]) -> String {
    hex::encode(&blake3::hash(public_key).as_bytes()[..16])
}

/// What a backup server's refusal means, in words for the Backups panel and the alert. 413 is one
/// piece too big to pass (a web server in front with less than the 6 MB a piece needs, or the
/// relay's own object limit), NOT a full server: it was worded as "no more room" until 2026-10-25,
/// when nginx's 1 MB default on both servers' /api/ sent the operator to raise a space limit that
/// was never the problem. 507 is the keeping server out of room (its disk, or the space given).
pub fn refusal_words(status: u16, said: &str) -> String {
    let with_said = |s: &str| if s.is_empty() { String::new() } else { format!(": {s}") };
    match status {
        403 => format!("the backup server does not keep backups for this server: ask its admin to add this server's key (HTTP 403{})", with_said(said)),
        401 => "the backup server did not accept this server's signature (check both clocks)".to_string(),
        413 => format!("the backup server turned away one piece as too big (HTTP 413{}): a piece is up to about 4 MB, so the web server in front of it must let /api/backup-host/ take 6 MB (client_max_body_size, docs/admin/continuous-backup.md step 2)", with_said(said)),
        507 => format!("the backup server has no more room for this server's backups (HTTP 507{}): ask its admin for a bigger limit, or keep less history", with_said(said)),
        _ => format!("the backup server refused (HTTP {status}{})", with_said(said)),
    }
}

pub struct ServerDest {
    base: String,
    kp: DilithiumKeypair,
    fp: String,
    client: reqwest::Client,
    /// Each request's time is later than the one before, so no two share a time (the host
    /// refuses a time it has seen for the same request).
    last_ms: std::sync::atomic::AtomicU64,
}

impl ServerDest {
    /// `seed` is this server's identity seed (`Storage::get_or_create_server_keypair`).
    pub fn new(url: &str, seed: &[u8; 32]) -> Result<Self, String> {
        let kp = DilithiumKeypair::from_seed(&derive_dilithium_seed(seed));
        let fp = fingerprint(&kp.public_key());
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(15))
            .timeout(std::time::Duration::from_secs(180))
            .build()
            .map_err(|e| format!("could not start the web client: {e}"))?;
        Ok(Self { base: url.trim().trim_end_matches('/').to_string(), kp, fp, client, last_ms: Default::default() })
    }

    fn now_ms(&self) -> u64 {
        use std::sync::atomic::Ordering;
        let wall = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
        let mut prev = self.last_ms.load(Ordering::SeqCst);
        loop {
            let next = wall.max(prev + 1);
            match self.last_ms.compare_exchange(prev, next, Ordering::SeqCst, Ordering::SeqCst) {
                Ok(_) => return next,
                Err(p) => prev = p,
            }
        }
    }

    fn signed(&self, req: reqwest::RequestBuilder, op: &str, name: &str, body: &[u8]) -> reqwest::RequestBuilder {
        let ms = self.now_ms();
        let sig = hex::encode(self.kp.sign(preimage(op, name, body, ms).as_bytes()));
        req.header(H_FROM, &self.fp).header(H_TIME, ms.to_string()).header(H_SIG, sig)
    }

    async fn send(&self, req: reqwest::RequestBuilder) -> Result<reqwest::Response, String> {
        let r = req.send().await.map_err(|e| {
            if e.is_timeout() {
                "the backup server took too long to answer".to_string()
            } else {
                "could not reach the backup server".to_string()
            }
        })?;
        let status = r.status().as_u16();
        if r.status().is_success() || status == 404 {
            return Ok(r);
        }
        let said = r.text().await.unwrap_or_default();
        let said: String = said.chars().take(200).collect();
        Err(refusal_words(status, &said))
    }

    pub async fn put(&self, name: &str, bytes: Vec<u8>) -> Result<(), String> {
        let req = self.signed(self.client.put(format!("{}/api/backup-host/o/{name}", self.base)), "put", name, &bytes).body(bytes);
        let r = self.send(req).await?;
        if r.status().as_u16() == 404 {
            return Err("the backup server does not offer to keep backups (it needs a HumanityOS release from October 2026 or later)".into());
        }
        Ok(())
    }

    pub async fn get(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        let req = self.signed(self.client.get(format!("{}/api/backup-host/o/{name}", self.base)), "get", name, &[]);
        let r = self.send(req).await?;
        if r.status().as_u16() == 404 {
            return Ok(None);
        }
        r.bytes().await.map(|b| Some(b.to_vec())).map_err(|_| "the backup server's answer was cut off".to_string())
    }

    pub async fn delete(&self, name: &str) -> Result<(), String> {
        let req = self.signed(self.client.delete(format!("{}/api/backup-host/o/{name}", self.base)), "delete", name, &[]);
        self.send(req).await.map(|_| ())
    }

    pub async fn list(&self) -> Result<Vec<Listed>, String> {
        let req = self.signed(self.client.get(format!("{}/api/backup-host/list", self.base)), "list", "", &[]);
        let r = self.send(req).await?;
        if r.status().as_u16() == 404 {
            return Err("the backup server does not offer to keep backups (it needs a HumanityOS release from October 2026 or later)".into());
        }
        let v: serde_json::Value = r.json().await.map_err(|_| "the backup server's list was not readable".to_string())?;
        Ok(v["objects"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|o| Some(Listed { name: o["name"].as_str()?.to_string(), size: o["size"].as_u64().unwrap_or(0) }))
                    .collect()
            })
            .unwrap_or_default())
    }
}
