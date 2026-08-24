//! Local DM history store — the client-side half of sealed-sender DMs
//! (2026-08-23).
//!
//! The relay's dm_mailbox is a delivery window, not an archive: envelopes
//! expire after `dm_mailbox_ttl_days` and carry no sender. Long-term DM
//! history therefore lives HERE, on the user's own device, encrypted at
//! rest with a key derived from their seed (so history is only readable
//! while the identity is unlocked, and a copied file is useless without
//! the seed).
//!
//! One store file per (identity, server): mailbox row ids are per-relay,
//! so the fetch high-water mark must not leak across servers.
//!
//! File format: 12-byte AES-GCM nonce ‖ AES-256-GCM(ciphertext of the
//! JSON body). Key = BLAKE3.derive_key("hum/dm-store/v1", seed).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng as AesOsRng},
    AeadCore, Aes256Gcm, Key, Nonce,
};
use serde::{Deserialize, Serialize};

use super::dm_pq::DmInner;

/// BLAKE3 domain for the at-rest encryption key. Distinct from every
/// other seed-derived key (identity, kyber, dm-aes).
const STORE_KEY_DOMAIN: &str = "hum/dm-store/v1";

/// One stored message. `dedupe` is the envelope's inner-signature hash —
/// the same message arriving twice (live echo + fetch, or a replay) is
/// dropped by it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredDm {
    pub from: String,
    pub to: String,
    pub ts: u64,
    pub text: String,
    pub dedupe: String,
}

/// The serialized body (what gets encrypted into the file).
#[derive(Debug, Default, Serialize, Deserialize)]
struct StoreBody {
    /// Highest mailbox row id fetched from this server.
    high_water: i64,
    /// peer identity hex → messages, kept sorted by ts.
    conversations: HashMap<String, Vec<StoredDm>>,
    /// peer identity hex → last-read message ts (drives unread dots).
    last_read: HashMap<String, u64>,
    // ── Client-side social graph (follows removal, 2026-08-24). The
    // server stores no edges; these sets ARE the user's social state,
    // built from sealed control messages. serde defaults keep stores
    // from before this change loading cleanly.
    /// Keys I follow.
    #[serde(default)]
    following: std::collections::HashSet<String>,
    /// Keys that follow me (learned from [[hum:follow]] notices).
    #[serde(default)]
    followers: std::collections::HashSet<String>,
    /// peer → certificate THEY issued authorizing ME to DM them
    /// (presented on every dm_put to that peer).
    #[serde(default)]
    certs_from: HashMap<String, String>,
    /// Peers I've already issued MY certificate to (dedupe).
    #[serde(default)]
    certs_sent: std::collections::HashSet<String>,
}

/// A conversation summary for the sidebar.
#[derive(Debug, Clone)]
pub struct DmConversationSummary {
    pub peer: String,
    pub last_text: String,
    pub last_ts: u64,
    pub last_from_me: bool,
    pub unread: bool,
}

pub struct DmStore {
    path: PathBuf,
    key: [u8; 32],
    /// Our own identity hex — decides which side of a message is "the peer".
    me: String,
    body: StoreBody,
    seen: HashSet<String>,
}

impl DmStore {
    /// Directory for DM stores: `%APPDATA%/HumanityOS/dms/` (same base the
    /// config and saves use), falling back to `./dms` in portable setups.
    fn store_dir() -> PathBuf {
        if let Ok(appdata) = std::env::var("APPDATA") {
            PathBuf::from(appdata).join("HumanityOS").join("dms")
        } else if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home).join(".humanityos").join("dms")
        } else {
            PathBuf::from("dms")
        }
    }

    /// Load (or create empty) the store for this identity on this server.
    pub fn load(seed: &[u8], identity_hex: &str, server_url: &str) -> Self {
        let key = blake3::derive_key(STORE_KEY_DOMAIN, seed);
        // Filename: hash of identity+server so neither appears on disk in
        // the clear (the directory listing itself shouldn't map users to
        // servers).
        let tag = blake3::hash(format!("{identity_hex}\n{server_url}").as_bytes());
        let path = Self::store_dir().join(format!("{}.dmstore", &tag.to_hex()[..24]));
        let mut store = Self {
            path,
            key,
            me: identity_hex.to_string(),
            body: StoreBody::default(),
            seen: HashSet::new(),
        };
        if let Ok(raw) = std::fs::read(&store.path) {
            if raw.len() > 12 {
                let (nonce_bytes, ct) = raw.split_at(12);
                let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&store.key));
                if let Ok(plain) = cipher.decrypt(Nonce::from_slice(nonce_bytes), ct) {
                    if let Ok(body) = serde_json::from_slice::<StoreBody>(&plain) {
                        store.body = body;
                    }
                }
                // A decrypt/parse failure (corrupt file or foreign seed)
                // starts an empty store rather than crashing chat; the
                // server window will refill recent history.
            }
        }
        for msgs in store.body.conversations.values() {
            for m in msgs {
                store.seen.insert(m.dedupe.clone());
            }
        }
        store
    }

    /// Persist to disk (encrypt-then-write, atomic via temp rename).
    pub fn save(&self) {
        let Ok(json) = serde_json::to_vec(&self.body) else { return };
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        let nonce = Aes256Gcm::generate_nonce(&mut AesOsRng);
        let Ok(ct) = cipher.encrypt(&nonce, json.as_slice()) else { return };
        let mut out = Vec::with_capacity(12 + ct.len());
        out.extend_from_slice(nonce.as_slice());
        out.extend_from_slice(&ct);
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = self.path.with_extension("tmp");
        if std::fs::write(&tmp, &out).is_ok() {
            let _ = std::fs::rename(&tmp, &self.path);
        }
    }

    pub fn high_water(&self) -> i64 {
        self.body.high_water
    }

    /// Raise the fetch high-water mark (never lowers it).
    pub fn set_high_water(&mut self, id: i64) {
        if id > self.body.high_water {
            self.body.high_water = id;
        }
    }

    /// Which conversation a verified message belongs to, from OUR side.
    pub fn peer_of(&self, inner: &DmInner) -> String {
        if inner.from == self.me { inner.to.clone() } else { inner.from.clone() }
    }

    /// Insert a verified message. Returns false if it was a duplicate.
    pub fn insert(&mut self, inner: &DmInner) -> bool {
        let dedupe = inner.dedupe_key();
        if !self.seen.insert(dedupe.clone()) {
            return false;
        }
        let peer = self.peer_of(inner);
        let list = self.body.conversations.entry(peer).or_default();
        list.push(StoredDm {
            from: inner.from.clone(),
            to: inner.to.clone(),
            ts: inner.ts,
            text: inner.text.clone(),
            dedupe,
        });
        // Keep sorted by the signed timestamp (arrival order can differ).
        list.sort_by_key(|m| m.ts);
        true
    }

    /// All messages of one conversation, oldest first.
    pub fn conversation(&self, peer: &str) -> &[StoredDm] {
        self.body
            .conversations
            .get(peer)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Mark a conversation read up to `ts`.
    pub fn mark_read(&mut self, peer: &str, ts: u64) {
        let cur = self.body.last_read.entry(peer.to_string()).or_insert(0);
        if ts > *cur {
            *cur = ts;
        }
    }

    /// Does `peer` have messages from them newer than our last read?
    pub fn has_unread(&self, peer: &str) -> bool {
        let read_ts = self.body.last_read.get(peer).copied().unwrap_or(0);
        self.conversation(peer)
            .iter()
            .any(|m| m.from != self.me && m.ts > read_ts)
    }

    /// Sidebar summaries, newest conversation first.
    pub fn conversations(&self) -> Vec<DmConversationSummary> {
        let mut out: Vec<DmConversationSummary> = self
            .body
            .conversations
            .iter()
            .filter_map(|(peer, msgs)| {
                let last = msgs.last()?;
                Some(DmConversationSummary {
                    peer: peer.clone(),
                    last_text: last.text.clone(),
                    last_ts: last.ts,
                    last_from_me: last.from == self.me,
                    unread: self.has_unread(peer),
                })
            })
            .collect();
        out.sort_by(|a, b| b.last_ts.cmp(&a.last_ts));
        out
    }

    // ── Client-side social graph (follows removal, 2026-08-24) ──

    pub fn set_following(&mut self, peer: &str, on: bool) {
        if on {
            self.body.following.insert(peer.to_string());
        } else {
            self.body.following.remove(peer);
        }
    }
    pub fn is_following(&self, peer: &str) -> bool {
        self.body.following.contains(peer)
    }
    pub fn following(&self) -> &std::collections::HashSet<String> {
        &self.body.following
    }
    pub fn set_follower(&mut self, peer: &str, on: bool) {
        if on {
            self.body.followers.insert(peer.to_string());
        } else {
            self.body.followers.remove(peer);
        }
    }
    pub fn is_follower(&self, peer: &str) -> bool {
        self.body.followers.contains(peer)
    }
    pub fn followers(&self) -> &std::collections::HashSet<String> {
        &self.body.followers
    }
    /// Friends = mutual follow (the UI notion; the transport credential
    /// is the certificate, tracked separately).
    pub fn is_friend(&self, peer: &str) -> bool {
        self.is_following(peer) && self.is_follower(peer)
    }
    /// The certificate `peer` issued authorizing ME to DM them.
    pub fn cert_for(&self, peer: &str) -> Option<&str> {
        self.body.certs_from.get(peer).map(|s| s.as_str())
    }
    pub fn store_cert_from(&mut self, peer: &str, cert: &str) {
        self.body.certs_from.insert(peer.to_string(), cert.to_string());
    }
    pub fn cert_sent_to(&self, peer: &str) -> bool {
        self.body.certs_sent.contains(peer)
    }
    pub fn mark_cert_sent(&mut self, peer: &str) {
        self.body.certs_sent.insert(peer.to_string());
    }

    /// Delete one whole conversation locally (the server holds nothing to
    /// delete beyond the TTL window; dm_purge covers that separately).
    pub fn delete_conversation(&mut self, peer: &str) {
        if let Some(msgs) = self.body.conversations.remove(peer) {
            for m in msgs {
                self.seen.remove(&m.dedupe);
            }
        }
        self.body.last_read.remove(peer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::dm_pq;
    use crate::relay::core::pq_crypto;

    fn identity(seed_byte: u8) -> (Vec<u8>, String) {
        let seed = vec![seed_byte; 32];
        let dil_seed = pq_crypto::derive_dilithium_seed(&seed);
        let hex = hex::encode(pq_crypto::DilithiumKeypair::from_seed(&dil_seed).public_key());
        (seed, hex)
    }

    fn temp_server() -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        format!("wss://test-{nanos}.example")
    }

    #[test]
    fn roundtrip_persist_and_reload_encrypted() {
        let (alice_seed, alice_hex) = identity(31);
        let (_bob_seed, bob_hex) = identity(32);
        let server = temp_server();
        let inner_json =
            dm_pq::build_signed_inner(&alice_seed, &alice_hex, &bob_hex, 100, "persist me").unwrap();
        let inner = dm_pq::parse_verify_inner(&inner_json).unwrap();

        let mut store = DmStore::load(&alice_seed, &alice_hex, &server);
        assert!(store.insert(&inner));
        assert!(!store.insert(&inner), "duplicate must be rejected");
        store.set_high_water(7);
        store.save();

        // Reload with the right seed: everything is back.
        let store2 = DmStore::load(&alice_seed, &alice_hex, &server);
        assert_eq!(store2.high_water(), 7);
        assert_eq!(store2.conversation(&bob_hex).len(), 1);
        assert_eq!(store2.conversation(&bob_hex)[0].text, "persist me");

        // The file on disk is ciphertext — the plaintext never appears.
        let raw = std::fs::read(&store2.path).unwrap();
        let needle = b"persist me";
        assert!(
            !raw.windows(needle.len()).any(|w| w == needle),
            "store file must not contain plaintext"
        );

        // A different seed cannot read it (fresh empty store, no crash).
        let (eve_seed, _) = identity(33);
        let store3 = DmStore::load(&eve_seed, &alice_hex, &server);
        assert_eq!(store3.conversation(&bob_hex).len(), 0);

        let _ = std::fs::remove_file(&store2.path);
    }

    #[test]
    fn unread_tracks_peer_messages_only() {
        let (alice_seed, alice_hex) = identity(41);
        let (bob_seed, bob_hex) = identity(42);
        let server = temp_server();
        let mut store = DmStore::load(&alice_seed, &alice_hex, &server);

        // Alice's own outgoing message never counts as unread.
        let mine = dm_pq::parse_verify_inner(
            &dm_pq::build_signed_inner(&alice_seed, &alice_hex, &bob_hex, 10, "sent").unwrap(),
        )
        .unwrap();
        store.insert(&mine);
        assert!(!store.has_unread(&bob_hex));

        // Bob's incoming message does, until marked read.
        let theirs = dm_pq::parse_verify_inner(
            &dm_pq::build_signed_inner(&bob_seed, &bob_hex, &alice_hex, 20, "reply").unwrap(),
        )
        .unwrap();
        store.insert(&theirs);
        assert!(store.has_unread(&bob_hex));
        store.mark_read(&bob_hex, 20);
        assert!(!store.has_unread(&bob_hex));

        let convs = store.conversations();
        assert_eq!(convs.len(), 1);
        assert_eq!(convs[0].last_text, "reply");
        assert!(!convs[0].last_from_me);
        let _ = std::fs::remove_file(&store.path);
    }
}
