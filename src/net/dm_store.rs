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
use super::reach::{ContactRequest, FriendTicks, ReachSettings};

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
    /// The sender's base64 Dilithium3 signature over the DM v2 preimage, kept since step D
    /// (2026-10-09, blocking-and-safe-mode.md 10e) so a message can be handed to a server's
    /// admins as evidence they can check (net/report.rs). A message kept before then has none
    /// and is not offered as evidence.
    #[serde(default)]
    pub sig: String,
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
    // ── Friendship passes v2 (2026-10-09, blocking-and-safe-mode.md 10b). The JSON keys are
    // new on purpose: a store written in the v1 days had `certs_from` / `certs_sent` holding
    // v1 certificates, which no relay accepts any more. Under new names those simply are not
    // read, so the old records are gone on the next save and the pass sweep
    // (engine/dm.rs `sweep_friend_passes`) mints v2 passes for every current mutual follow.
    /// The did:hum of the server the passes below were given on (its `identify_challenge`
    /// says). A pass names its server, so if this server's identity ever changes, every pass
    /// below is void and is dropped (`set_pass_server`).
    #[serde(default)]
    pass_server: String,
    /// peer → the pass THEY gave ME (the v2 JSON as it arrived, checked then), presented
    /// whenever I reach them: every dm_put, trade request, call ring and direct offer.
    #[serde(default, rename = "passes_from")]
    certs_from: HashMap<String, String>,
    /// peer → the passes I gave THEM that still stand: the serial (what a withdrawal names)
    /// and what each allows. Usually one; two devices minting at once can make two.
    #[serde(default, rename = "passes_sent")]
    certs_sent: HashMap<String, Vec<SentPass>>,
    /// Serials of passes I withdrew that the relay has not confirmed yet (`cert_revoked`).
    /// Resent on every connection until it does, so going offline cannot lose a withdrawal.
    #[serde(default)]
    withdrawals_pending: Vec<String>,
    // ── Who can reach me (step B, 2026-10-09, blocking-and-safe-mode.md 10c) ──
    /// What this server last said our audiences are (`reach_settings`). Kept so that a DM
    /// fetched on the next connection, before the server has answered again, is judged by the
    /// person's real choice rather than by the defaults.
    #[serde(default)]
    reach_settings: Option<ReachSettings>,
    /// The Requests list: contact requests, and DMs our own settings would have refused (name
    /// only, never their text). Kept until Accept or Ignore.
    #[serde(default)]
    requests: Vec<ContactRequest>,
    /// Each friend's ticks in "People I choose" (10c-ii, 2026-10-10): Message, Call and Trade.
    /// The pass they hold from us is re-issued to allow exactly what is ticked. A friend with
    /// no entry has the defaults (`FriendTicks::default`), and an entry equal to the defaults is
    /// not kept, so "no choice" has one spelling. Step B's `may_call` set, which this replaced,
    /// is simply not read any more (no installed base to carry over, CLAUDE.md).
    #[serde(default)]
    friend_ticks: HashMap<String, FriendTicks>,
    // ── Reports about my groups (10j, 2026-10-10, blocking-and-safe-mode.md) ──
    /// Reports members of a group I created sent me, each item already checked against my own
    /// copy of the group, newest last. Kept here, encrypted, until I dismiss them; never sent to
    /// any server. Per server, like the groups themselves.
    #[serde(default)]
    group_reports: Vec<super::group_report::KeptReport>,
    /// Reports that arrived and still wait for their check (it needs this server's group list
    /// and the group's messages). Kept so closing the app before the check cannot lose one: the
    /// mailbox has already moved past it.
    #[serde(default)]
    group_reports_pending: Vec<super::group_report::PendingReport>,
}

/// A friendship pass I gave someone: what a withdrawal names, and what it allows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SentPass {
    pub serial: String,
    pub may: String,
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
    /// The block list (net/block_list.rs) keeps its own file here too.
    pub(crate) fn store_dir() -> PathBuf {
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
            sig: inner.sig_b64.clone(),
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
    // ── Friendship passes v2 (2026-10-09) ──

    /// The did:hum of the server these passes belong to, once a challenge has named it.
    pub fn pass_server(&self) -> Option<&str> {
        Some(self.body.pass_server.as_str()).filter(|s| !s.is_empty())
    }
    /// Record the server's did:hum from its `identify_challenge`. A different one than before
    /// voids every pass held or given here (they name the old identity) and every withdrawal
    /// still waiting (it would name passes this server never saw); the sweep then mints anew.
    /// Returns true when it changed.
    pub fn set_pass_server(&mut self, did: &str) -> bool {
        if did.is_empty() || self.body.pass_server == did {
            return false;
        }
        if !self.body.pass_server.is_empty() {
            self.body.certs_from.clear();
            self.body.certs_sent.clear();
            self.body.withdrawals_pending.clear();
        }
        self.body.pass_server = did.to_string();
        true
    }
    /// The pass `peer` gave ME, presented whenever I reach them.
    pub fn cert_for(&self, peer: &str) -> Option<&str> {
        self.body.certs_from.get(peer).map(|s| s.as_str())
    }
    pub fn store_cert_from(&mut self, peer: &str, cert: &str) {
        self.body.certs_from.insert(peer.to_string(), cert.to_string());
    }
    /// Forget the pass `peer` gave me (they unfollowed, and so withdrew it).
    pub fn forget_cert_from(&mut self, peer: &str) {
        self.body.certs_from.remove(peer);
    }
    /// Have I a pass standing with `peer`?
    pub fn cert_sent_to(&self, peer: &str) -> bool {
        self.body.certs_sent.get(peer).is_some_and(|v| !v.is_empty())
    }
    /// The passes I gave `peer` that still stand.
    pub fn passes_sent_to(&self, peer: &str) -> &[SentPass] {
        self.body.certs_sent.get(peer).map(|v| v.as_slice()).unwrap_or(&[])
    }
    /// Record a pass I gave `peer` (minted here, or echoed from my other device). Once per serial.
    pub fn record_pass_sent(&mut self, peer: &str, pass: SentPass) {
        let list = self.body.certs_sent.entry(peer.to_string()).or_default();
        if !list.iter().any(|p| p.serial == pass.serial) {
            list.push(pass);
        }
    }
    /// Take back every pass I gave `peer`: they leave the record and their serials join the
    /// withdrawals waiting for the relay. Returns the serials withdrawn now.
    pub fn withdraw_passes_to(&mut self, peer: &str) -> Vec<String> {
        let gone: Vec<String> = self.body.certs_sent.remove(peer).unwrap_or_default().into_iter().map(|p| p.serial).collect();
        for s in &gone {
            if !self.body.withdrawals_pending.contains(s) {
                self.body.withdrawals_pending.push(s.clone());
            }
        }
        gone
    }
    /// Withdrawals the relay has not confirmed yet.
    pub fn pending_withdrawals(&self) -> &[String] {
        &self.body.withdrawals_pending
    }
    /// The relay confirmed a withdrawal (`cert_revoked {serial}`): stop resending it.
    pub fn withdrawal_confirmed(&mut self, serial: &str) {
        self.body.withdrawals_pending.retain(|s| s != serial);
    }
    /// Mutual follows I have no standing pass with: the ones the sweep mints for (after the v2
    /// change, after a server identity change, or when a send could not go out earlier).
    pub fn friends_without_pass(&self) -> Vec<String> {
        let mut out: Vec<String> = self.body.following.iter().filter(|p| self.is_follower(p) && !self.cert_sent_to(p)).cloned().collect();
        out.sort();
        out
    }

    /// Take back every pass I gave `peer` that `keep` does not keep: they leave the record and
    /// their serials join the waiting withdrawals. Used once a re-issued pass has gone out (keep
    /// the new serial) and when our other device's re-issue is echoed (keep its `may`).
    pub fn withdraw_passes_to_except(&mut self, peer: &str, keep: impl Fn(&SentPass) -> bool) -> Vec<String> {
        let Some(list) = self.body.certs_sent.get_mut(peer) else { return Vec::new() };
        let gone: Vec<String> = list.iter().filter(|p| !keep(p)).map(|p| p.serial.clone()).collect();
        list.retain(|p| keep(p));
        if list.is_empty() {
            self.body.certs_sent.remove(peer);
        }
        for s in &gone {
            if !self.body.withdrawals_pending.contains(s) {
                self.body.withdrawals_pending.push(s.clone());
            }
        }
        gone
    }

    // ── Who can reach me (step B, 2026-10-09) ──

    /// What this server last said our audiences are, if it has said.
    pub fn reach_settings(&self) -> Option<ReachSettings> {
        self.body.reach_settings
    }
    /// Keep what the server says (`reach_settings`): the only source of what the Safety page shows.
    pub fn set_reach_settings(&mut self, settings: ReachSettings) {
        self.body.reach_settings = Some(settings);
    }
    /// The Requests list, oldest first.
    pub fn requests(&self) -> &[ContactRequest] {
        &self.body.requests
    }
    /// File a request: one entry per person (by key), so a repeat moves its time on and keeps the
    /// pass a contact request brought (a later refused DM brings none), and never makes a second
    /// row. Returns true when it is a new entry.
    pub fn add_request(&mut self, req: ContactRequest) -> bool {
        if let Some(have) = self.body.requests.iter_mut().find(|r| r.key == req.key) {
            have.ts = have.ts.max(req.ts);
            if !req.pass.is_empty() {
                have.pass = req.pass;
            }
            return false;
        }
        self.body.requests.push(req);
        true
    }
    /// Take `key`'s request off the list (Accept or Ignore), returning it.
    pub fn remove_request(&mut self, key: &str) -> Option<ContactRequest> {
        let at = self.body.requests.iter().position(|r| r.key == key)?;
        Some(self.body.requests.remove(at))
    }
    /// `peer`'s ticks in "People I choose" (10c-ii): what the person chose, or the defaults.
    pub fn ticks(&self, peer: &str) -> FriendTicks {
        self.body.friend_ticks.get(peer).copied().unwrap_or_default()
    }
    /// Keep the person's choice for `peer`. A choice equal to the defaults is the same as none,
    /// so it is not stored (a friend with no entry has the defaults).
    pub fn set_ticks(&mut self, peer: &str, ticks: FriendTicks) {
        if ticks == FriendTicks::default() {
            self.body.friend_ticks.remove(peer);
        } else {
            self.body.friend_ticks.insert(peer.to_string(), ticks);
        }
    }
    /// Forget the person's choice for `peer`, back to the defaults (our Unfollow, and Block,
    /// 10c-ii). True when there was one to forget.
    pub fn clear_ticks(&mut self, peer: &str) -> bool {
        self.body.friend_ticks.remove(peer).is_some()
    }
    /// Who the "People I choose" list shows (10c-ii, sorted by key; the page sorts by name):
    /// everyone holding a pass from us, the spec's "someone I have given a pass", which also takes
    /// in someone we sent a contact request to. Plus every mutual follow, who is owed one: the
    /// pass sweep gives each a pass, and while it cannot yet (no DM key for them, or a pass taken
    /// back at once by an untick made offline, engine/dm.rs `reissue_pass`) they stay on the list
    /// rather than vanishing the moment their ticks are changed.
    pub fn people_to_choose(&self) -> Vec<String> {
        let mut out: Vec<String> = self.body.certs_sent.iter().filter(|(_, v)| !v.is_empty()).map(|(k, _)| k.clone()).collect();
        for peer in &self.body.following {
            if self.is_follower(peer) && !out.contains(peer) {
                out.push(peer.clone());
            }
        }
        out.sort();
        out
    }
    /// The `may` a pass to `peer` should carry, canonical form.
    pub fn intended_may_wire(&self, peer: &str) -> String {
        super::reach::intended_may_wire(self.ticks(peer))
    }
    /// People holding a pass from us that does not allow what the person chose for them (a tick
    /// added or taken away whose re-issue could not go out yet): the ones the pass sweep
    /// re-issues for.
    pub fn passes_out_of_step(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .body
            .certs_sent
            .iter()
            .filter(|(peer, passes)| {
                let want = self.intended_may_wire(peer);
                passes.iter().any(|p| p.may != want)
            })
            .map(|(peer, _)| peer.clone())
            .collect();
        out.sort();
        out
    }
    /// The "show as a request" rule (10c) for a DM from `from`: does our message setting (as
    /// this server last said it, or the safe defaults) let them through? `shares_group` is
    /// whether they are in a P2P group with us.
    pub fn admits_dm_from(&self, from: &str, shares_group: bool) -> bool {
        let rel = super::reach::Relation { is_me: from == self.me, passes: self.passes_sent_to(from), shares_group };
        !super::reach::shows_as_request(&self.body.reach_settings.unwrap_or_default(), &rel)
    }

    // ── Reports about my groups (10j) ──

    /// Keep a report until its check: false when this report (by its id) is already waiting or
    /// kept, so a mailbox that hands it over again changes nothing.
    pub fn add_pending_group_report(&mut self, p: super::group_report::PendingReport) -> bool {
        if self.body.group_reports_pending.iter().any(|x| x.id == p.id) || self.body.group_reports.iter().any(|x| x.id == p.id) {
            return false;
        }
        self.body.group_reports_pending.push(p);
        true
    }
    /// The reports waiting for their check.
    pub fn pending_group_reports(&self) -> &[super::group_report::PendingReport] {
        &self.body.group_reports_pending
    }
    /// A check finished: the report leaves the waiting list and, when it was kept, joins the
    /// kept ones. True when a report was kept now (the caller says so once).
    pub fn settle_group_report(&mut self, id: &str, kept: Option<super::group_report::KeptReport>) -> bool {
        self.body.group_reports_pending.retain(|p| p.id != id);
        match kept {
            Some(k) if !self.body.group_reports.iter().any(|x| x.id == k.id) => {
                self.body.group_reports.push(k);
                true
            }
            _ => false,
        }
    }
    /// Every kept report, newest first.
    pub fn group_reports(&self) -> Vec<&super::group_report::KeptReport> {
        let mut out: Vec<&super::group_report::KeptReport> = self.body.group_reports.iter().collect();
        out.sort_by(|a, b| b.ts.cmp(&a.ts));
        out
    }
    /// One kept report.
    pub fn group_report(&self, id: &str) -> Option<&super::group_report::KeptReport> {
        self.body.group_reports.iter().find(|r| r.id == id)
    }
    /// Dismiss: the report is gone from this device. True when there was one.
    pub fn remove_group_report(&mut self, id: &str) -> bool {
        let before = self.body.group_reports.len();
        self.body.group_reports.retain(|r| r.id != id);
        self.body.group_reports.len() != before
    }
    /// Mark that the person reported was removed from the group.
    pub fn set_group_report_removed(&mut self, id: &str) -> bool {
        match self.body.group_reports.iter_mut().find(|r| r.id == id) {
            Some(r) => {
                r.removed = true;
                true
            }
            None => false,
        }
    }
    /// How many kept reports are about this group (the count on it in the group list).
    pub fn group_report_count(&self, group_id: &str) -> usize {
        super::group_report::count(&self.body.group_reports, group_id)
    }

    /// For tests elsewhere in the crate: remove this store's file from disk.
    #[cfg(test)]
    pub(crate) fn remove_file_for_test(&self) {
        let _ = std::fs::remove_file(&self.path);
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

    /// Write `json` as the store's body, encrypted as `save` does (to stand in for a store an
    /// older version of the app wrote).
    fn write_body(store: &DmStore, json: &serde_json::Value) {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&store.key));
        let nonce = Aes256Gcm::generate_nonce(&mut AesOsRng);
        let ct = cipher.encrypt(&nonce, json.to_string().as_bytes()).unwrap();
        let mut out = nonce.as_slice().to_vec();
        out.extend_from_slice(&ct);
        std::fs::create_dir_all(store.path.parent().unwrap()).unwrap();
        std::fs::write(&store.path, out).unwrap();
    }

    /// FRIENDSHIP PASSES v2 (2026-10-09). A store written in the v1 days loads with its history
    /// and follows intact, and with its v1 certificates gone (no relay accepts them now), so
    /// every mutual follow is listed for a new pass. Passes given are recorded by serial;
    /// withdrawing moves the serials to the waiting list until the relay confirms each; a new
    /// server identity voids every pass and waiting withdrawal.
    /// Seen red 2026-10-09 with the `rename = "passes_from"` taken off `certs_from`: "the v1
    /// certificate is not read", left `Some("djEtY2VydA==")`.
    #[test]
    fn passes_v2_replace_v1_records_and_track_withdrawals() {
        let (seed, me) = identity(51);
        let server = temp_server();
        let store = DmStore::load(&seed, &me, &server);
        write_body(&store, &serde_json::json!({
            "high_water": 9, "conversations": {}, "last_read": {},
            "following": ["ann", "ben", "cy"], "followers": ["ann", "ben"],
            "certs_from": { "ann": "djEtY2VydA==" }, "certs_sent": ["ann", "ben"],
        }));
        let mut store = DmStore::load(&seed, &me, &server);
        assert_eq!(store.high_water(), 9, "the rest of the store loads");
        assert!(store.is_friend("ann") && store.is_friend("ben") && !store.is_friend("cy"));
        assert_eq!(store.cert_for("ann"), None, "the v1 certificate is not read");
        assert_eq!(store.friends_without_pass(), vec!["ann".to_string(), "ben".to_string()], "both friends get a new pass");
        assert_eq!(store.pass_server(), None);

        assert!(store.set_pass_server("did:hum:one"));
        assert!(!store.set_pass_server("did:hum:one"), "the same server again changes nothing");
        let p = |s: &str| SentPass { serial: s.into(), may: "invite,message,trade,voice_message".into() };
        store.record_pass_sent("ann", p("aa"));
        store.record_pass_sent("ann", p("aa"));
        store.record_pass_sent("ann", p("ab"));
        store.store_cert_from("ann", "{\"v\":2}");
        assert_eq!(store.passes_sent_to("ann").len(), 2, "recorded once per serial");
        assert_eq!(store.friends_without_pass(), vec!["ben".to_string()]);
        store.save();
        let mut store = DmStore::load(&seed, &me, &server);
        assert_eq!(store.passes_sent_to("ann").len(), 2, "kept across a restart");
        assert_eq!(store.pass_server(), Some("did:hum:one"));

        assert_eq!(store.withdraw_passes_to("ann"), vec!["aa".to_string(), "ab".to_string()]);
        assert!(!store.cert_sent_to("ann"));
        assert_eq!(store.pending_withdrawals(), ["aa".to_string(), "ab".to_string()]);
        store.withdrawal_confirmed("aa");
        assert_eq!(store.pending_withdrawals(), ["ab".to_string()], "confirmed ones stop");

        store.record_pass_sent("ben", p("bb"));
        assert!(store.set_pass_server("did:hum:two"), "a new server identity");
        assert!(store.passes_sent_to("ben").is_empty() && store.cert_for("ann").is_none() && store.pending_withdrawals().is_empty(), "voids every pass");
        assert_eq!(store.friends_without_pass(), vec!["ann".to_string(), "ben".to_string()]);
        let _ = std::fs::remove_file(&store.path);
    }

    /// "People I choose" (10c-ii): a friend's ticks are kept across a restart; a friend with no
    /// choice, or a choice equal to the defaults, has the defaults and no entry; a pass that does
    /// not match the ticks is listed for re-issue; the list shows everyone holding a pass from us
    /// and every mutual follow, once each; clearing forgets the choice. A store written with step
    /// B's `may_call` set loads, and that set is not read (no migration).
    /// Seen red 2026-10-10 with `friend_ticks` marked `#[serde(skip)]` (kept in memory, never
    /// saved): "kept across a restart" failed (Ben read back as the defaults); and with
    /// `people_to_choose` listing mutual follows only, as step B's list did: "pass holders and
    /// mutual follows, once each" failed (Cy, who holds a pass, missing).
    #[test]
    fn friend_ticks_are_kept_and_step_b_may_call_is_not_read() {
        let (seed, me) = identity(61);
        let server = temp_server();
        let store = DmStore::load(&seed, &me, &server);
        write_body(&store, &serde_json::json!({
            "high_water": 3, "conversations": {}, "last_read": {},
            "following": ["ann", "ben"], "followers": ["ann", "ben"], "may_call": ["ann"],
        }));
        let mut store = DmStore::load(&seed, &me, &server);
        assert_eq!(store.high_water(), 3, "a store with the old field loads");
        assert_eq!(store.ticks("ann"), FriendTicks::default(), "the old may_call set is not read");

        let chosen = FriendTicks { message: false, call: true, trade: true };
        store.set_ticks("ben", chosen);
        store.set_ticks("ann", FriendTicks::default());
        assert!(!store.clear_ticks("ann"), "a choice equal to the defaults is not stored");
        let default_may = super::super::reach::intended_may_wire(FriendTicks::default());
        store.record_pass_sent("ann", SentPass { serial: "aa".repeat(16), may: default_may.clone() });
        store.record_pass_sent("ben", SentPass { serial: "bb".repeat(16), may: default_may });
        assert_eq!(store.passes_out_of_step(), vec!["ben".to_string()], "Ben's pass does not match his ticks yet");

        // The list: everyone holding a pass from us (Cy, sent one with a contact request) and
        // every mutual follow (Eve, whose pass has not gone out yet); not someone we only follow.
        store.record_pass_sent("cy", SentPass { serial: "cc".repeat(16), may: "invite,message,trade,voice_message".into() });
        for (peer, follows_us) in [("dee", false), ("eve", true)] {
            store.set_following(peer, true);
            store.set_follower(peer, follows_us);
        }
        assert_eq!(store.people_to_choose(), ["ann", "ben", "cy", "eve"], "pass holders and mutual follows, once each");
        store.save();

        let mut store = DmStore::load(&seed, &me, &server);
        assert_eq!(store.ticks("ben"), chosen, "kept across a restart");
        assert_eq!(store.intended_may_wire("ben"), "call,trade");
        assert!(store.clear_ticks("ben"), "clearing forgets the choice");
        assert_eq!(store.ticks("ben"), FriendTicks::default(), "back to the defaults");
        assert!(store.passes_out_of_step().is_empty(), "and Ben's default pass is in step again");
        let _ = std::fs::remove_file(&store.path);
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
