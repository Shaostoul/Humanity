//! Hide and keep in the desktop app (2026-11-20, docs/design/hide-and-keep.md rung 5): the client's
//! rules, with no socket and no GUI so each one is unit tested. The screens are
//! src/gui/pages/server_settings/held.rs (Held copies) and server_settings/reports.rs (the "Hide and
//! keep" decision); the frames and the background jobs are src/engine/held.rs. The website's twin
//! is web/chat/chat-held.js with web/shared/held-seal.js.
//!
//! THE PROTOCOL (the relay's half is src/relay/handlers/held.rs):
//! - `{"type":"report_decide","id","decision":"hide_and_keep","note","action":<signed>}` decides a
//!   report, `action` the decider's own `mod_action_v1` (`hide_and_keep` naming the report);
//! - `{"type":"held_list"}` asks for Held copies, answered (admins only) with `held_items`: whether
//!   this server can keep a copy and if not why (the relay's words), whether I am the owner, each
//!   item's numbers and dates (never what it holds; its package id only to the owner whose key it is
//!   sealed to, who needs it to open the parts), the signed action log and the counts;
//! - `{"type":"held_act","action":<signed>}` carries the owner's legal hold, destroy now, export or
//!   restore; an export or a restore grants the owner's device ten minutes to fetch the sealed parts
//!   (`GET /api/held/{id}/{part}?grant=<the action's object id>`);
//! - a restore then uploads each file the owner's device opened (`PUT` of that part's address with
//!   the header `x-held-restore-key`, the key part 0 carries) and finishes with
//!   `POST /api/held/{id}/restore?grant=` (the same header): the relay checks every byte against
//!   the plan it sealed under that key, puts the post back and deletes the held copy.
//!
//! NEVER ON SCREEN. Nothing here returns a part's plaintext to a caller that could draw it: Export
//! writes it straight into the .zip file the owner chose ([`export_with`]), Restore streams it
//! straight back up ([`held_seal::restore_with`] with [`upload_part`]).

use std::io::{Read, Seek, SeekFrom, Write};

use humanity_protocol::held_seal::{self, open_manifest, send_in_pieces, Opener};
use humanity_protocol::pq_crypto::KyberKeypair;
use serde_json::Value;

/// The furthest ahead a legal hold may end, in days (the relay's `LEGAL_HOLD_MAX_DAYS`).
pub const HOLD_MAX_DAYS: i64 = 366;
/// A day in milliseconds.
pub const DAY_MS: i64 = 86_400_000;
/// The header a restore's uploads and its finish carry the restore key in.
pub const RESTORE_KEY_HEADER: &str = "x-held-restore-key";

/// One held item as an admin is shown it: numbers and dates, nothing of what it holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeldItem {
    pub id: i64,
    pub report_id: i64,
    pub file_count: i64,
    pub parts: i64,
    pub sealed_bytes: i64,
    pub held_at: i64,
    pub held_by_name: String,
    pub delete_after: i64,
    pub legal_hold_until: i64,
    /// When the expiry pass deletes it: its year or its legal hold's end, whichever is later.
    pub deleted_at: i64,
    pub exported_count: i64,
    /// Sealed to the owner's key now (false: to an earlier owner's, so nobody here can open it).
    pub sealed_to_owner: bool,
    /// Its package id: sent only to the owner whose key it is sealed to, never drawn.
    pub package: String,
    /// Also sealed to the successor the owner named (2026-11-20, net/owner.rs).
    pub sealed_to_successor: bool,
    /// The post's author (key) and its time (Unix ms), as the list showed them: what an Export or a
    /// Restore opens must be this very post (core `held_seal::manifest_is`, the fourth review's
    /// finding 8: one report can hold several posts, each its own item).
    pub author: String,
    pub author_name: String,
    pub stamp: i64,
}

/// One signed action in the log.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeldLogLine {
    pub action: String,
    pub target: i64,
    pub signer_name: String,
    pub reason: String,
    pub until: i64,
    pub signed_at: i64,
}

/// How many of each action this server carried out, ever.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HeldCounts {
    pub held: i64,
    pub exported: i64,
    pub legal_holds: i64,
    pub destroyed: i64,
    pub restored: i64,
    /// Hides whose sealed copy waits for an owner change to end (the relay's words in Held copies).
    pub waiting: i64,
    /// Waiting folders a stop left with no note and no post the relay could name.
    pub unidentified: i64,
}

/// The `held_items` frame, read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeldList {
    pub can_keep: bool,
    pub cannot_keep_why: String,
    pub i_am_owner: bool,
    pub items: Vec<HeldItem>,
    pub actions: Vec<HeldLogLine>,
    pub counts: HeldCounts,
    /// The successor the owner named, if any (every admin sees who), and the owner log: each
    /// claim, handover and change of successor (net/owner.rs).
    pub successor: Option<super::owner::Successor>,
    pub owner_log: Vec<super::owner::OwnerLogLine>,
}

/// What the desktop app keeps for hide and keep (`GuiState::reports.held`).
#[derive(Debug, Default)]
pub struct HeldUi {
    /// The list the server last sent.
    pub list: Option<HeldList>,
    /// A `held_list` was sent on this socket; cleared on a new socket so the page asks again.
    pub requested: bool,
    /// One line: the relay's answer to the last action, a job's words, or why nothing was sent.
    pub status: String,
    /// The reason typed for each item's next action (every action is signed with its reason).
    pub reasons: std::collections::HashMap<i64, String>,
    /// The legal hold's days typed for each item.
    pub days: std::collections::HashMap<i64, i64>,
    /// The question open on an item: Destroy now or Restore.
    pub confirm: Option<(i64, Confirm)>,
    /// An Export or Restore signed and sent, waiting for the relay's grant, then running.
    pub pending: Option<Pending>,
    /// The running job's answer.
    pub running: Option<std::sync::mpsc::Receiver<Result<String, String>>>,
    /// From the server's last word on it (the Reports list or Held copies): whether hide and keep
    /// can seal a copy here, and if not, the relay's own sentence why.
    pub keep: Option<(bool, String)>,
}

impl HeldUi {
    /// A new socket or another server: everything here was that server's.
    pub fn forget_server(&mut self) {
        *self = HeldUi::default();
    }
}

/// The question an action asks first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirm {
    Destroy,
    Restore,
}

/// Which job the owner's device runs once granted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobKind {
    Export,
    Restore,
}

/// An Export or Restore signed and sent.
#[derive(Debug, Clone)]
pub struct Pending {
    pub kind: JobKind,
    pub item: HeldItem,
    /// The signed action's object id: the grant the parts are fetched under.
    pub grant: String,
    /// Export: the new file the parts are opened into.
    pub file: Option<std::path::PathBuf>,
    pub sent_at_ms: i64,
    /// The relay granted it and the job is running.
    pub started: bool,
}

/// Whether a `private` line from the relay is about held items (its answer to a held action or
/// a refusal of one), so Held copies shows it as well as Chat.
pub fn is_held_words(message: &str) -> bool {
    ["Held item #", "Export of held item", "Restore of held item", "The legal hold on held item", "There is no held item", "Only admins can see held items", "Nothing was done:"]
        .iter()
        .any(|p| message.starts_with(p))
        || message.contains("This action must be signed by your own key")
}

fn int(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(Value::as_i64).unwrap_or(0)
}
fn text(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

/// Read a `held_items` frame; None for any other frame.
pub fn parse_list(frame: &Value) -> Option<HeldList> {
    if frame.get("type").and_then(Value::as_str) != Some("held_items") {
        return None;
    }
    let list = |k: &str| frame.get(k).and_then(Value::as_array).cloned().unwrap_or_default();
    let items = list("items")
        .iter()
        .map(|i| HeldItem {
            id: int(i, "id"),
            report_id: int(i, "report_id"),
            file_count: int(i, "file_count"),
            parts: int(i, "parts"),
            sealed_bytes: int(i, "sealed_bytes"),
            held_at: int(i, "held_at"),
            held_by_name: text(i, "held_by_name"),
            delete_after: int(i, "delete_after"),
            legal_hold_until: int(i, "legal_hold_until"),
            deleted_at: int(i, "deleted_at"),
            exported_count: int(i, "exported_count"),
            sealed_to_owner: i.get("sealed_to_owner").and_then(Value::as_bool).unwrap_or(false),
            package: text(i, "package"),
            sealed_to_successor: i.get("sealed_to_successor").and_then(Value::as_bool).unwrap_or(false),
            author: text(i, "author"),
            author_name: text(i, "author_name"),
            stamp: int(i, "stamp"),
        })
        .collect();
    let actions = list("actions")
        .iter()
        .map(|a| HeldLogLine {
            action: text(a, "action"),
            target: int(a, "target"),
            signer_name: text(a, "signer_name"),
            reason: text(a, "reason"),
            until: int(a, "until"),
            signed_at: int(a, "signed_at"),
        })
        .collect();
    let c = frame.get("counts").cloned().unwrap_or(Value::Null);
    Some(HeldList {
        can_keep: frame.get("can_keep").and_then(Value::as_bool).unwrap_or(false),
        cannot_keep_why: text(frame, "cannot_keep_why"),
        i_am_owner: frame.get("i_am_owner").and_then(Value::as_bool).unwrap_or(false),
        items,
        actions,
        counts: HeldCounts { held: int(&c, "held"), exported: int(&c, "exported"), legal_holds: int(&c, "legal_holds"), destroyed: int(&c, "destroyed"), restored: int(&c, "restored"), waiting: int(&c, "waiting"), unidentified: int(&c, "unidentified") },
        successor: super::owner::parse_successor(frame),
        owner_log: super::owner::parse_log(frame),
    })
}

/// A signed held action: its object id (the grant an export or a restore fetches with) and the
/// submission JSON the relay checks (the shape `POST /api/v2/objects` takes).
#[derive(Debug, Clone)]
pub struct Signed {
    pub object_id: String,
    pub submission: Value,
}

/// Sign `action` on `target` (a report's number for `hide_and_keep`, a held item's for the rest)
/// as the person whose seed this is, for the server `server_did`, at `now_ms`. `until` is a legal
/// hold's end (0 for none and for every other action).
pub fn sign(seed: &[u8], server_did: &str, action: &str, target: i64, reason: &str, until: u64, now_ms: u64) -> Result<Signed, String> {
    use humanity_protocol::moderation::{build_mod_action_v1, held_target_kind, ModAction};
    let kind = held_target_kind(action).ok_or_else(|| format!("{action} is not one of hide and keep's actions"))?;
    if server_did.is_empty() {
        return Err("no server identity".into());
    }
    let a = ModAction { action, target: &target.to_string(), target_kind: kind, reason: reason.trim(), rule: "", expires_at: until, role: "" };
    let o = build_mod_action_v1(seed, server_did, &a, now_ms, &[]).map_err(|e| e.to_string())?;
    signed_of(&o)
}

/// A signed object as the relay takes it: its object id and the submission JSON (the shape
/// `POST /api/v2/objects` takes). The owner's actions (net/owner.rs) are carried the same way.
pub fn signed_of(o: &humanity_protocol::object::Object) -> Result<Signed, String> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let object_id = o.object_id().map_err(|e| format!("{e:?}"))?.to_hex();
    let mut submission = serde_json::json!({
        "protocol_version": o.protocol_version,
        "object_type": o.object_type,
        "author_public_key_b64": B64.encode(&o.author_public_key),
        "references": o.references,
        "payload_schema_version": o.payload_schema_version,
        "payload_encoding": o.payload_encoding,
        "payload_b64": B64.encode(&o.payload),
        "signature_b64": B64.encode(&o.signature),
    });
    if let Some(s) = &o.space_id {
        submission["space_id"] = Value::String(s.clone());
    }
    if let Some(c) = &o.channel_id {
        submission["channel_id"] = Value::String(c.clone());
    }
    if let Some(t) = o.created_at {
        submission["created_at"] = Value::from(t);
    }
    Ok(Signed { object_id, submission })
}

/// The `report_decide` frame of a hide and keep, carrying its signed action.
pub fn decide_frame(report_id: &Value, note: &str, signed: &Signed) -> String {
    serde_json::json!({ "type": "report_decide", "id": report_id, "decision": "hide_and_keep", "note": note, "action": signed.submission }).to_string()
}

/// The `held_act` frame carrying an owner's signed action.
pub fn act_frame(signed: &Signed) -> String {
    serde_json::json!({ "type": "held_act", "action": signed.submission }).to_string()
}

/// The `held_list` frame.
pub fn list_frame() -> String {
    serde_json::json!({ "type": "held_list" }).to_string()
}

/// A legal hold's end for `days` from `now_ms` (0 lifts the hold), or None outside 0 to
/// [`HOLD_MAX_DAYS`].
pub fn hold_until(days: i64, now_ms: i64) -> Option<u64> {
    match days {
        0 => Some(0),
        d if (1..=HOLD_MAX_DAYS).contains(&d) => Some((now_ms + d * DAY_MS) as u64),
        _ => None,
    }
}

/// Where part `part` of held item `id` is fetched (and, on a restore, uploaded) under `grant`.
pub fn part_url(base: &str, id: i64, part: usize, grant: &str) -> String {
    format!("{}/api/held/{id}/{part}?grant={grant}", base.trim_end_matches('/'))
}

/// Where a restore finishes.
pub fn restore_url(base: &str, id: i64, grant: &str) -> String {
    format!("{}/api/held/{id}/restore?grant={grant}", base.trim_end_matches('/'))
}

// ── Export: one .zip file the owner chose ──

/// A name safe inside the export: no folders, no characters a system refuses, never empty (the
/// website's `safeEntryName`).
pub fn safe_entry_name(name: &str) -> String {
    let last = name.rsplit(['/', '\\']).next().unwrap_or("");
    let mut out: String = last.chars().map(|c| if c.is_control() || "<>:\"|?*".contains(c) { '_' } else { c }).collect();
    let dots = out.len() - out.trim_start_matches('.').len();
    if dots > 0 {
        out = format!("_{}", &out[dots..]);
    }
    let out: String = out.chars().take(120).collect();
    if out.is_empty() {
        "file".into()
    } else {
        out
    }
}

/// The file name an export is offered under in the folder the owner chose; a number is added when
/// one is there already, so nothing is overwritten.
pub fn export_path(folder: &std::path::Path, id: i64) -> std::path::PathBuf {
    let first = folder.join(format!("held-item-{id}.zip"));
    if !first.exists() {
        return first;
    }
    (2..1000).map(|n| folder.join(format!("held-item-{id}-{n}.zip"))).find(|p| !p.exists()).unwrap_or(first)
}

const CRC_POLY: u32 = 0xedb8_8320;

/// CRC-32 as the zip format uses it, carried across calls (start with 0).
pub fn crc32_update(crc: u32, bytes: &[u8]) -> u32 {
    let mut c = !crc;
    for &b in bytes {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { CRC_POLY ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

/// A .zip written an entry at a time, every entry stored (no compression library, and every system
/// opens it): each entry's bytes stream in from a reader, and its CRC and size are written back
/// into its header once known.
pub struct ZipOut<W: Write + Seek> {
    out: W,
    central: Vec<u8>,
    count: u16,
}

impl<W: Write + Seek> ZipOut<W> {
    pub fn new(out: W) -> Self {
        Self { out, central: Vec::new(), count: 0 }
    }

    /// Add `name` with everything `src` holds; the bytes it took.
    pub fn add(&mut self, name: &str, mut src: impl Read) -> std::io::Result<u64> {
        let start = self.out.stream_position()?;
        let name_b = name.as_bytes();
        let header = |crc: u32, size: u32| -> Vec<u8> {
            let mut h = Vec::with_capacity(30 + name_b.len());
            h.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
            h.extend_from_slice(&Self::common(crc, size, name_b.len()));
            h.extend_from_slice(name_b);
            h
        };
        self.out.write_all(&header(0, 0))?;
        let (mut crc, mut size, mut buf) = (0u32, 0u64, vec![0u8; 64 * 1024]);
        loop {
            let n = src.read(&mut buf)?;
            if n == 0 {
                break;
            }
            crc = crc32_update(crc, &buf[..n]);
            size += n as u64;
            if size > u32::MAX as u64 - 1 {
                return Err(std::io::Error::other("a file larger than 4 GB cannot go into this export"));
            }
            self.out.write_all(&buf[..n])?;
        }
        let end = self.out.stream_position()?;
        self.out.seek(SeekFrom::Start(start))?;
        self.out.write_all(&header(crc, size as u32))?;
        self.out.seek(SeekFrom::Start(end))?;
        self.central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        self.central.extend_from_slice(&20u16.to_le_bytes());
        self.central.extend_from_slice(&Self::common(crc, size as u32, name_b.len()));
        // The comment's length, the disk, the internal (2) and external (4) attributes: all zero.
        self.central.extend_from_slice(&[0u8; 10]);
        self.central.extend_from_slice(&(start as u32).to_le_bytes());
        self.central.extend_from_slice(name_b);
        self.count += 1;
        Ok(size)
    }

    /// The fields a local header and a central entry share: version 20, the name in UTF-8, stored,
    /// 1 January 1980.
    fn common(crc: u32, size: u32, name_len: usize) -> Vec<u8> {
        let mut c = Vec::with_capacity(26);
        for v in [20u16, 0x0800, 0, 0, 0x21] {
            c.extend_from_slice(&v.to_le_bytes());
        }
        c.extend_from_slice(&crc.to_le_bytes());
        c.extend_from_slice(&size.to_le_bytes());
        c.extend_from_slice(&size.to_le_bytes());
        c.extend_from_slice(&(name_len as u16).to_le_bytes());
        c.extend_from_slice(&0u16.to_le_bytes());
        c
    }

    /// Write the central directory and the end record; the writer back.
    pub fn finish(mut self) -> std::io::Result<W> {
        let at = self.out.stream_position()?;
        self.out.write_all(&self.central)?;
        let mut end = Vec::with_capacity(22);
        end.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        end.extend_from_slice(&[0u8; 4]);
        end.extend_from_slice(&self.count.to_le_bytes());
        end.extend_from_slice(&self.count.to_le_bytes());
        end.extend_from_slice(&(self.central.len() as u32).to_le_bytes());
        end.extend_from_slice(&(at as u32).to_le_bytes());
        end.extend_from_slice(&0u16.to_le_bytes());
        self.out.write_all(&end)?;
        self.out.flush()?;
        Ok(self.out)
    }
}

/// What an export wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exported {
    pub path: std::path::PathBuf,
    pub files: usize,
}

/// Export held item `id` (package `package`) into a new file at `path`, opening every part with
/// `kyber` on this device. `fetch(part)` reads a sealed part (from the server, under the export's
/// grant). The file holds `held-item.json` (the list of files and who and when, never the restore
/// key), `post.txt` (the words) and `files/<part>-<name>`; any failure removes the file, so a half
/// export is never left behind. It is created new: an existing file is never written over.
///
/// The opened manifest must be the item the owner chose, its package, report, author and post time
/// as the list showed them (core `held_seal::manifest_is`; the third review's W7, the fourth
/// review's finding 8); otherwise nothing is written.
pub fn export_with(kyber: &KyberKeypair, chosen: &held_seal::Chosen, path: &std::path::Path, now_ms: i64, mut fetch: impl FnMut(usize) -> Result<Box<dyn Read>, String>) -> Result<Exported, String> {
    let package = chosen.package;
    let manifest = open_manifest(kyber, package, fetch(0)?)?;
    held_seal::manifest_is(&manifest, chosen)?;
    let file = std::fs::OpenOptions::new().write(true).create_new(true).open(path).map_err(|e| format!("the file could not be made ({e})"))?;
    let result = (|| -> Result<usize, String> {
        let mut zip = ZipOut::new(std::io::BufWriter::new(file));
        let mut about = serde_json::to_value(&manifest).map_err(|e| e.to_string())?;
        if let Some(o) = about.as_object_mut() {
            o.remove("restore_key");
            o.remove("words");
            o.insert("exported_at".into(), Value::from(now_ms));
        }
        let io = |e: std::io::Error| format!("the file could not be written ({e})");
        zip.add("held-item.json", serde_json::to_string_pretty(&about).unwrap_or_default().as_bytes()).map_err(io)?;
        zip.add("post.txt", manifest.words.as_bytes()).map_err(io)?;
        let mut files = 0;
        for f in &manifest.files {
            let name = safe_entry_name(if f.original_name.is_empty() { &f.name } else { &f.original_name });
            let mut parts = vec![(f.part, format!("files/{}-{name}", f.part))];
            if let Some(p) = f.poster_part {
                parts.push((p, format!("files/{p}-poster-{name}.jpg")));
            }
            for (part, entry) in parts {
                let opener = Opener::new(kyber, &held_seal::part_context(package, part), fetch(part)?)?;
                zip.add(&entry, opener).map_err(|e| format!("part {part} could not be opened into the file ({e})"))?;
            }
            files += 1;
        }
        let mut w = zip.finish().map_err(io)?;
        w.flush().map_err(io)?;
        let f = w.into_inner().map_err(|e| format!("the file could not be written ({e})"))?;
        f.sync_all().map_err(io)?;
        Ok(files)
    })();
    match result {
        Ok(files) => Ok(Exported { path: path.to_path_buf(), files }),
        Err(e) => {
            let _ = std::fs::remove_file(path);
            Err(e)
        }
    }
}

// ── Restore: opened here, uploaded again ──
//
// The order (part 0 for the restore key, each file and poster part, then finish) is the protocol's
// `held_seal::restore_with`, which engine/held.rs runs with this file's fetch, upload and finish.

/// An answer's body as JSON (Null when it is not JSON).
fn json_of(r: ureq::Response) -> Value {
    r.into_string().ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null)
}

/// Who asks the held part routes (the owner review's finding 1): this app's key, and the seed that
/// signs each request (core `held_seal::caller_purpose`, the relay's signed-request pattern). A
/// grant alone fetches nothing; the relay serves a part only to the key the grant belongs to.
#[derive(Clone)]
pub struct Caller {
    pub key: String,
    seed: Vec<u8>,
}

impl std::fmt::Debug for Caller {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Caller").field("key", &self.key).finish_non_exhaustive() // never the seed
    }
}

impl Caller {
    pub fn new(key: &str, seed: &[u8]) -> Self {
        Caller { key: key.to_string(), seed: seed.to_vec() }
    }

    /// The three headers proving this caller for `purpose` at the server `opened` (the URL the
    /// request goes to), signed now (a time later than any earlier request's, so a retried request
    /// is never taken for a replay). The signature is the signed requests' (net/identity.rs
    /// `pq_sign_request`: `hum/request/v2`, the server's address, the purpose, the time; merged
    /// 2026-11-22), so it is good at that server alone and no chat line can equal it; until then
    /// it was `purpose\ntime`, a chat line's shape (the owner branch's review, S6).
    pub fn headers(&self, opened: &str, purpose: &str) -> [(&'static str, String); 3] {
        use humanity_protocol::held_seal::{CALLER_KEY, CALLER_SIG, CALLER_TIME};
        let ts = next_ms();
        [(CALLER_KEY, self.key.clone()), (CALLER_TIME, ts.to_string()), (CALLER_SIG, crate::net::identity::pq_sign_request(&self.seed, opened, purpose, ts))]
    }

    /// `req` signed for `purpose`, at the server its URL names.
    pub fn sign(&self, req: ureq::Request, purpose: &str) -> ureq::Request {
        let opened = req.url().to_string();
        self.headers(&opened, purpose).iter().fold(req, |r, (k, v)| r.set(k, v))
    }
}

/// Unix ms, strictly later than the last call's.
fn next_ms() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static LAST: AtomicU64 = AtomicU64::new(0);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    let mut prev = LAST.load(Ordering::SeqCst);
    loop {
        let next = now.max(prev + 1);
        match LAST.compare_exchange(prev, next, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => return next,
            Err(p) => prev = p,
        }
    }
}

/// A sealed part from the server, under `grant`, asked by `caller`: a reader over the answer's body.
pub fn fetch_part(base: &str, id: i64, part: usize, grant: &str, caller: &Caller) -> Result<Box<dyn Read>, String> {
    let purpose = humanity_protocol::held_seal::caller_purpose("part", grant, id, part as i64, 0);
    match caller.sign(ureq::get(&part_url(base, id, part, grant)), &purpose).call() {
        Ok(r) => Ok(Box::new(r.into_reader())),
        Err(ureq::Error::Status(_, _)) => Err("the server did not grant this (its answer is in the chat)".into()),
        Err(e) => Err(format!("the server could not be reached ({e})")),
    }
}

/// The most one restore piece carries: the relay's `RESTORE_PIECE_MAX`, under nginx's 1 MB limit
/// on `/api/`.
pub const RESTORE_PIECE: usize = 768 * 1024;

/// Upload a part the owner's device opened, a piece at a time as it is opened, on a restore.
pub fn upload_part(base: &str, id: i64, part: usize, grant: &str, key: &str, caller: &Caller, src: &mut dyn Read) -> Result<(), String> {
    let url = part_url(base, id, part, grant);
    let purpose = |offset| humanity_protocol::held_seal::caller_purpose("restore", grant, id, part as i64, offset);
    send_in_pieces(src, RESTORE_PIECE, |offset, piece| match caller.sign(ureq::put(&format!("{url}&offset={offset}")), &purpose(offset)).set(RESTORE_KEY_HEADER, key).send_bytes(piece) {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(_, r)) => Err(json_of(r).get("error").and_then(Value::as_str).unwrap_or_default().to_string()),
        Err(e) => Err(format!("the server could not be reached ({e})")),
    })
}

/// Finish a restore: the relay puts the post back; its words.
pub fn finish_restore(base: &str, id: i64, grant: &str, key: &str, caller: &Caller) -> Result<String, String> {
    let words = |v: Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let purpose = humanity_protocol::held_seal::caller_purpose("finish", grant, id, 0, 0);
    match caller.sign(ureq::post(&restore_url(base, id, grant)), &purpose).set(RESTORE_KEY_HEADER, key).call() {
        Ok(r) => Ok(words(json_of(r), "words")),
        Err(ureq::Error::Status(_, r)) => Err(words(json_of(r), "error")),
        Err(e) => Err(format!("the server could not be reached ({e})")),
    }
}

#[cfg(test)]
#[path = "held_tests.rs"]
pub(crate) mod tests;
