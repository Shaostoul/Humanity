//! The organisations an identity belongs to, kept on this device (docs/design/organisations.md,
//! section 3; rung 1, 2026-11-14). No drawing: the page is src/gui/pages/orgs.rs, the rules are
//! `humanity_protocol::org`.
//!
//! WHERE THEY LIVE. One encrypted file per identity, `<data folder>/orgs/<hash>.orgs` (beside
//! config.json), written crash-safely (`humanity_core::atomic_write::write_atomic`): AES-256-GCM
//! with a random 96-bit nonce per save, under BLAKE3 `derive_key("hum/orgs/v1", seed)` (its own
//! domain; docs/reference/cryptography.md, "Organisations"), the file named by a hash of the
//! identity, so a copied file names nobody and says nothing without the seed. Inside, JSON
//! `{"v": 1, "orgs": [<carried text of each organisation>], "outbox": [...]}`: each organisation's log
//! in the very shape a person carries (`org::bundle_text`), and (rung 2) the deliveries not sent yet
//! (`gui::org_mail::OutItem`, an optional field: a body without it opens with nothing waiting). No
//! server holds any of it.
//!
//! A FILE THAT DOES NOT OPEN IS NEVER WRITTEN OVER (the rule of src/gui/mine_tasks.rs): it is
//! renamed aside to `<name>.unreadable-<unix seconds>` and the page says so.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng as AesOsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use humanity_protocol::object::Object;
use humanity_protocol::org;

use crate::gui::org_mail::OutItem;

/// The key-derivation domain of the file's key.
const FILE_KEY_DOMAIN: &str = "hum/orgs/v1";
/// The body's version.
const BODY_VERSION: u64 = 1;

/// Why the file on disk could not be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreProblem {
    /// It did not open (damaged, or another identity's) and was renamed to this path.
    KeptAside(PathBuf),
    /// It could not be read or set aside; this session does not save over it.
    Unreadable(String),
}

/// Every organisation log this identity keeps.
pub struct OrgStore {
    path: Option<PathBuf>,
    key: [u8; 32],
    logs: BTreeMap<String, Vec<Object>>,
    /// Deliveries not sent yet (rung 2), oldest first.
    outbox: Vec<OutItem>,
    /// Bumped on every change: the page refolds an organisation when it moves.
    version: u64,
    problem: Option<StoreProblem>,
}

/// The organisations in a body's JSON text (`{"v": 1, "orgs": [...]}`), each as (id, objects).
pub fn parse_body(text: &str) -> Option<BTreeMap<String, Vec<Object>>> {
    parse_body_full(text).map(|(logs, _)| logs)
}

/// The logs and the deliveries waiting in a body's JSON text.
fn parse_body_full(text: &str) -> Option<(BTreeMap<String, Vec<Object>>, Vec<OutItem>)> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    if v["v"].as_u64() != Some(BODY_VERSION) {
        return None;
    }
    let mut logs = BTreeMap::new();
    for bundle in v["orgs"].as_array()? {
        let (id, objects) = org::parse_bundle(&bundle.to_string()).ok()?;
        let mut log = Vec::new();
        org::merge(&id, &mut log, objects);
        logs.insert(id, log);
    }
    let outbox = v["outbox"].as_array().map(|a| a.iter().filter_map(OutItem::from_json).collect()).unwrap_or_default();
    Some((logs, outbox))
}

fn body_text(logs: &BTreeMap<String, Vec<Object>>, outbox: &[OutItem]) -> String {
    let orgs: Vec<serde_json::Value> = logs
        .iter()
        .filter_map(|(id, objs)| serde_json::from_str::<serde_json::Value>(&org::bundle_text(id, objs)).ok())
        .collect();
    let outbox: Vec<serde_json::Value> = outbox.iter().map(OutItem::to_json).collect();
    serde_json::json!({ "v": BODY_VERSION, "orgs": orgs, "outbox": outbox }).to_string()
}

fn set_aside(path: &Path) -> StoreProblem {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let aside = path.with_file_name(format!("{name}.unreadable-{secs}"));
    match std::fs::rename(path, &aside) {
        Ok(()) => {
            log::warn!("[orgs] the organisations file did not open; kept aside as {}", aside.display());
            StoreProblem::KeptAside(aside)
        }
        Err(e) => {
            log::warn!("[orgs] the organisations file did not open and could not be set aside ({e}); not saving over it");
            StoreProblem::Unreadable(e.to_string())
        }
    }
}

impl OrgStore {
    /// This identity's store from its file (an empty one when there is none yet).
    pub fn load(seed: &[u8], identity_hex: &str) -> Self {
        let tag = blake3::hash(format!("orgs\n{identity_hex}").as_bytes());
        let dir = crate::config::AppConfig::config_path().with_file_name("orgs");
        Self::load_at(seed, Some(dir.join(format!("{}.orgs", humanity_core::text::prefix(&tag.to_hex(), 24)))))
    }

    /// A store at `path` (`None`: kept in memory only, as for a test or no identity yet).
    pub fn load_at(seed: &[u8], path: Option<PathBuf>) -> Self {
        let mut s = Self { path, key: blake3::derive_key(FILE_KEY_DOMAIN, seed), logs: BTreeMap::new(), outbox: Vec::new(), version: 0, problem: None };
        let Some(path) = s.path.clone() else { return s };
        let raw = match std::fs::read(&path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return s,
            Err(e) => {
                s.problem = Some(StoreProblem::Unreadable(e.to_string()));
                return s;
            }
        };
        let opened = (raw.len() > 12)
            .then(|| {
                let (nonce, ct) = raw.split_at(12);
                Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&s.key)).decrypt(Nonce::from_slice(nonce), ct).ok()
            })
            .flatten()
            .and_then(|plain| String::from_utf8(plain).ok())
            .and_then(|text| parse_body_full(&text));
        match opened {
            Some((logs, outbox)) => {
                s.logs = logs;
                s.outbox = outbox;
            }
            None => s.problem = Some(set_aside(&path)),
        }
        s
    }

    pub fn problem(&self) -> Option<&StoreProblem> {
        self.problem.as_ref()
    }
    pub fn version(&self) -> u64 {
        self.version
    }
    /// The organisation ids kept here.
    pub fn ids(&self) -> Vec<String> {
        self.logs.keys().cloned().collect()
    }
    pub fn objects(&self, org_id: &str) -> &[Object] {
        self.logs.get(org_id).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Merge `objects` into organisation `org_id`'s log and save; how many were new.
    pub fn add(&mut self, org_id: &str, objects: Vec<Object>) -> usize {
        let log = self.logs.entry(org_id.to_string()).or_default();
        let added = org::merge(org_id, log, objects);
        if log.is_empty() {
            self.logs.remove(org_id);
        }
        if added > 0 {
            self.version += 1;
            self.save();
        }
        added
    }

    /// Forget an organisation on this device (the log stays with its other members).
    pub fn forget(&mut self, org_id: &str) {
        if self.logs.remove(org_id).is_some() {
            self.outbox.retain(|i| i.org != org_id);
            self.version += 1;
            self.save();
        }
    }

    /// Keep deliveries to send (saved at once, so a crash before the send loses nothing).
    pub fn queue(&mut self, items: Vec<OutItem>) {
        if items.is_empty() {
            return;
        }
        for it in items {
            if !self.outbox.contains(&it) {
                self.outbox.push(it);
            }
        }
        self.save();
    }

    /// The deliveries waiting, oldest first.
    pub fn outbox(&self) -> &[OutItem] {
        &self.outbox
    }

    /// Take the waiting delivery at `index` out (the caller sends it, or queues it again).
    pub fn take_out(&mut self, index: usize) -> Option<OutItem> {
        (index < self.outbox.len()).then(|| self.outbox.remove(index))
    }

    /// Save now (after [`take_out`] and the sends it led to).
    pub fn flush(&self) {
        self.save();
    }

    /// The objects of `org_id` whose ids are `ids`, in the log's order.
    pub fn pick(&self, org_id: &str, ids: &[String]) -> Vec<Object> {
        let want: std::collections::BTreeSet<&str> = ids.iter().map(String::as_str).collect();
        self.objects(org_id).iter().filter(|o| o.object_id().is_ok_and(|h| want.contains(h.to_hex().as_str()))).cloned().collect()
    }

    fn save(&self) {
        let Some(path) = self.path.as_ref() else { return };
        if matches!(self.problem, Some(StoreProblem::Unreadable(_))) {
            return;
        }
        let nonce = Aes256Gcm::generate_nonce(&mut AesOsRng);
        let Ok(ct) = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key)).encrypt(&nonce, body_text(&self.logs, &self.outbox).as_bytes()) else {
            return;
        };
        let mut bytes = nonce.as_slice().to_vec();
        bytes.extend_from_slice(&ct);
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = humanity_core::atomic_write::write_atomic(path, &bytes) {
            log::warn!("[orgs] could not save the organisations file: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        std::env::temp_dir().join(format!("hum-orgs-test-{tag}-{nanos}.orgs"))
    }

    fn frozen() -> (String, Vec<Object>, String) {
        let path = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("scripts/tests/fixtures/org-log-v1.json");
        let text = std::fs::read_to_string(path).expect("the frozen log");
        let (id, objs) = org::parse_bundle(&text).expect("parses");
        (id, objs, text)
    }

    /// The saved shape's guard (migration-guard style): a body holding a log saved by rung 1 (the
    /// frozen log, which is that shape) opens to the same objects, and a saved file reopens to
    /// what was put in, encrypted so it names nobody. Change the body's shape and this fails.
    ///
    /// Red first, 2026-11-14: with `BODY_VERSION` set to 2, "a body saved by rung 1 opens" failed.
    #[test]
    fn a_saved_organisation_still_opens() {
        let (id, objs, text) = frozen();
        let body = format!("{{\"v\":1,\"orgs\":[{text}]}}");
        let logs = parse_body(&body).expect("a body saved by rung 1 opens");
        assert_eq!(logs.get(&id).map(Vec::len), Some(objs.len()));

        let path = temp_path("roundtrip");
        let seed = [9u8; 32];
        let mut s = OrgStore::load_at(&seed, Some(path.clone()));
        assert_eq!(s.add(&id, objs.clone()), objs.len());
        assert_eq!(s.add(&id, objs.clone()), 0, "the same log twice adds nothing");
        let raw = std::fs::read(&path).unwrap();
        assert!(!String::from_utf8_lossy(&raw).contains("Cart crew"), "the file is encrypted");
        let back = OrgStore::load_at(&seed, Some(path.clone()));
        assert_eq!(back.problem(), None);
        assert_eq!(back.objects(&id).len(), objs.len());
        let _ = std::fs::remove_file(&path);
    }

    /// A file another identity saved does not open, and is set aside, never written over.
    #[test]
    fn a_file_that_does_not_open_is_never_written_over() {
        let (id, objs, _) = frozen();
        let path = temp_path("aside");
        let mut s = OrgStore::load_at(&[1u8; 32], Some(path.clone()));
        s.add(&id, objs);
        let other = OrgStore::load_at(&[2u8; 32], Some(path.clone()));
        let Some(StoreProblem::KeptAside(aside)) = other.problem().cloned() else { panic!("set aside") };
        assert!(aside.exists() && !path.exists());
        let _ = std::fs::remove_file(aside);
    }
}
