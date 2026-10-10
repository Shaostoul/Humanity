//! Reports the admins can check, at the relay (step D, 2026-10-09,
//! docs/design/blocking-and-safe-mode.md 10e, which is the specification: the message names and
//! shapes below are what both clients build against; section 8 is the design).
//!
//! A report names the reported person by identity key, gives a reason from
//! data/safety/report_reasons.json, an optional note, and the evidence the reporter chose. It is
//! signed by the reporter, so it is theirs and its evidence is the evidence they chose, and it
//! is checked here before it is kept (storage/reports.rs, `reports_v2`).
//!
//! THE MESSAGES (exact names; the clients build against these):
//! - client to relay, from the signed-in socket: `{"type":"report_v2","target","context",
//!   "reason","note","evidence":[...],"ts","sig"}` ([`handle_report`]). `context` is one of
//!   `dm`, `post`, `group`, `profile`; `note` at most [`NOTE_MAX_CHARS`] characters; `sig` the
//!   reporter's Dilithium3 signature over `"hum/report/v1\n{reporter}\n{target}\n{reason}\n
//!   {evidence_hash}\n{ts}"` (pq_crypto `report_preimage`), where `evidence_hash` is the
//!   lowercase hex BLAKE3 of the `evidence` value's JSON text exactly as it arrived (clients send
//!   compact JSON; [`raw_member`] finds that text in the frame) and `reporter` is the socket's
//!   key, never a field of the report.
//! - relay to the reporter on success: `{"type":"report_received","id"}`.
//! - admins and moderators: `{"type":"reports_list","state":"open"|"decided"}` is answered with
//!   `{"type":"reports","state","items":[...]}` ([`handle_list`], [`item_json`]); `state` echoes
//!   the list asked for, so an empty answer still says which list it is.
//! - admins and moderators: `{"type":"report_decide","id","decision","note"}` ([`handle_decide`]),
//!   carried out through the moderation path (msg_handlers.rs `mod_action_refusal` and
//!   `handle_mod_action`), so its rules hold: a moderator cannot ban, nor act on an admin, and so
//!   on.
//!
//! EVIDENCE, at most [`EVIDENCE_MAX_ITEMS`] items and [`EVIDENCE_MAX_BYTES`] of JSON text:
//! - `{"kind":"dm","from","to","ts","text","sig"}`: a direct message's verified inner payload, as
//!   the reporter's client holds it after opening the seal. `checked: true` when `from` is the
//!   reported person, `to` is the reporter, and the reported person's signature holds over the DM
//!   v2 words rebuilt from those facts (pq_crypto `verify_dm_inner`): proof that they wrote
//!   exactly that text and sent it to the reporter. Anything else is kept with `checked: false`.
//! - `{"kind":"post","from","timestamp"}`: a public post. When `from` is the reported person, the
//!   relay looks it up in `messages` and keeps the text it has (`checked: true`); a post it does
//!   not have, or someone else's, is kept with `checked: false` and no text.
//! - `{"kind":"group_text","from","ts","text"}`: from a P2P group, whose signature covers the
//!   encrypted group object, so it cannot be proved here: kept with `checked: false`.
//! An item of any other kind, or with a field missing, refuses the report. So does an item (or
//! the note) whose text holds an encrypted file's marker, any version of it, anywhere
//! ([`DM_FILE_MARKER_PREFIX`], the same rule as the web's `reportTextHasFile`): that marker
//! carries the key to the file, which could be abuse imagery that no volunteer admin may be
//! handed, and it cannot be cut out without breaking the sender's signature. The reporter is told
//! only [`FILE_IN_EVIDENCE`], and nothing of the report is kept.
//!
//! REFUSED, with a notice to the reporter alone and nothing kept: a bad signature, an unknown
//! reason, a report about yourself, more than [`REPORTS_PER_HOUR`] reports in an hour, a second
//! report of the same person within 24 hours, evidence carrying a file (above), and a report that
//! is malformed (no target, an unknown context, a long note, too much evidence, an item of an
//! unknown kind).
//!
//! WHO SEES WHAT. A report goes to admins and moderators only. Admins see who reported; a
//! moderator sees "a member", and a direct message's `to` and signature (which name the reporter
//! just as well) are left out of what a moderator is sent. The reported person is never told who
//! reported them. Online admins and moderators get a notice when a report arrives.
//!
//! ALWAYS ON. `report_v2`, `reports_list`, `report_decide` and their answers are in features.rs
//! `WS_ALWAYS_ON`: reporting is a person's own protection and reviewing reports is the owner
//! administering their own server, and harm can come through any part (a game, a trade, a voice
//! room), so no choice of features switches either off.
//!
//! The old `/report <name> [reason]` (a typed name and a reason, nothing checkable) is gone; the
//! `/reports` command reads `reports_v2` ([`handle_slash`]).

use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;

use crate::relay::core::pq_crypto::{report_evidence_hash, verify_dm_inner, verify_report_sig};
use crate::relay::handlers::msg_handlers::{handle_mod_action, mod_action_refusal};
use crate::relay::relay::{RelayMessage, RelayState};
use crate::relay::storage::reports::{NewReport, ReportRow};

/// The longest note a report or a decision may carry, characters.
pub const NOTE_MAX_CHARS: usize = 500;
/// The most evidence items one report may carry.
pub const EVIDENCE_MAX_ITEMS: usize = 20;
/// The most evidence one report may carry: its `evidence` JSON text, bytes (64 KB).
pub const EVIDENCE_MAX_BYTES: usize = 64 * 1024;
/// Reports one reporter may file in an hour.
pub const REPORTS_PER_HOUR: usize = 3;
/// How long before the same reporter may report the same person again, ms (24 hours).
pub const SAME_TARGET_COOLDOWN_MS: i64 = 24 * 3_600_000;
/// Where a report was made.
pub const CONTEXTS: [&str; 4] = ["dm", "post", "group", "profile"];
/// What an admin or moderator may decide.
pub const DECISIONS: [&str; 6] = ["dismiss", "warn", "mute", "kick", "ban", "delete_post"];
/// The longest `target` read, characters (a Dilithium3 key is 3,904 hex characters).
const TARGET_MAX_LEN: usize = 4_096;
/// The most reports one `reports_list` answer carries.
const LIST_MAX: usize = 200;
/// The most reports the `/reports` command lists.
const SLASH_LIST_MAX: usize = 20;
/// What a moderator is shown for whoever reported.
const A_MEMBER: &str = "a member";
/// What an admin is shown for a reporter who has since erased their account.
const AN_ERASED_ACCOUNT: &str = "an erased account";

/// The start of every version of the marker an encrypted file rides in a sealed message under,
/// followed by the key that opens the file (today `[[hum:file:v1]]`: net::dm_pq `FILE_MARKER`, web
/// crypto.js `FILE_MARKER`; a test holds the native one to this). Text holding it anywhere is
/// treated as carrying a file, the web's `reportTextHasFile` rule, so all three parts agree and a
/// later version of the marker is caught too. Kept here because net::dm_pq is not in the relay
/// build.
pub const DM_FILE_MARKER_PREFIX: &str = "[[hum:file:";

/// The whole notice for a report whose evidence carries [`DM_FILE_MARKER_PREFIX`]. Such a message hands
/// whoever reads it the key to a file that could be abuse imagery; viewing or passing that on is
/// itself a crime, and reporting it belongs with the official reporting lines, not with a
/// server's volunteer admins. The marker cannot be cut out without breaking the sender's
/// signature, so the report is refused whole and nothing of it is kept (review of step D,
/// 2026-10-09).
pub const FILE_IN_EVIDENCE: &str = "A message carrying a file cannot be included in a report. If a child may be in danger, contact your local emergency number or your country's official reporting line.";

/// The answer to the old `/report <name>` command.
const REPORT_HOW: &str = "To report someone, use Report on one of their messages, in your conversation with them, or on their name in the member list. A report made there can carry their messages as evidence the admins can check. Nothing was sent.";

// ── The reasons (data/safety/report_reasons.json) ─────────────────────────

/// One reason a person may give, as the data file lists it.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReportReason {
    /// The word on the wire and in a stored report.
    pub id: String,
    /// What the report dialog shows.
    pub label: String,
    /// Shown under the reason when it is chosen, before sending.
    pub help: String,
}

/// Every reason this relay accepts, in the order the dialog lists them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReportReasons(pub Vec<ReportReason>);

impl ReportReasons {
    /// Read the reasons file's text: an ordered list of `{id, label, help}`, every field filled,
    /// each id a lowercase word (letters and `_`) used once.
    pub fn parse(text: &str) -> Result<ReportReasons, String> {
        let list: Vec<ReportReason> = serde_json::from_str(text).map_err(|e| format!("it is not a list of {{id, label, help}}: {e}"))?;
        if list.is_empty() {
            return Err("it lists no reasons".to_string());
        }
        for (i, r) in list.iter().enumerate() {
            if r.id.is_empty() || !r.id.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') {
                return Err(format!("reason {} has the id {:?}, which is not a lowercase word", i + 1, r.id));
            }
            if r.label.trim().is_empty() || r.help.trim().is_empty() {
                return Err(format!("reason {:?} has no label or no help", r.id));
            }
            if list[..i].iter().any(|o| o.id == r.id) {
                return Err(format!("the id {:?} is used twice", r.id));
            }
        }
        Ok(ReportReasons(list))
    }

    /// The reasons from data/safety/report_reasons.json, read when the relay starts.
    pub fn load() -> ReportReasons {
        Self::load_file(std::path::Path::new("data/safety/report_reasons.json"))
    }

    /// The reasons file at `path` (disk first, as every relay data file), else the copy built
    /// into the exe, said in the log (BUG-133), else none, which refuses every report as having
    /// an unknown reason and says so in the log.
    pub fn load_file(path: &std::path::Path) -> ReportReasons {
        let shown = path.display().to_string();
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
            crate::embedded_data::note_builtin_copy("safety/report_reasons.json", format_args!("{shown} could not be read ({e})"));
            include_str!("../../../data/safety/report_reasons.json").to_string()
        });
        match Self::parse(&text) {
            Ok(reasons) => reasons,
            Err(e) => {
                crate::embedded_data::note_builtin_copy("safety/report_reasons.json", format_args!("{shown} does not load ({e})"));
                Self::parse(include_str!("../../../data/safety/report_reasons.json")).unwrap_or_else(|e| {
                    tracing::error!("Reports: the built-in report_reasons.json does not load either ({e}); every report will be refused");
                    ReportReasons::default()
                })
            }
        }
    }

    /// The reason with this id.
    pub fn get(&self, id: &str) -> Option<&ReportReason> {
        self.0.iter().find(|r| r.id == id)
    }

    /// The label of `id`, or the id itself for one this relay no longer lists.
    pub fn label<'a>(&'a self, id: &'a str) -> &'a str {
        self.get(id).map_or(id, |r| r.label.as_str())
    }
}

// ── Small helpers ──────────────────────────────────────────────────────────

/// A notice to `who` alone, as a system line.
fn tell(state: &RelayState, who: &str, message: String) {
    let _ = state.broadcast_tx.send(RelayMessage::Private { to: who.to_string(), message });
}

fn role_of(state: &RelayState, key: &str) -> String {
    state.db.get_role(key).unwrap_or_default()
}

/// An admin or a moderator.
fn is_staff(role: &str) -> bool {
    matches!(role, "admin" | "mod" | "moderator")
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as i64
}

/// The registered name of `key`, or "".
fn name_of(state: &RelayState, key: &str) -> String {
    if key.is_empty() {
        return String::new();
    }
    state.db.name_for_key(key).ok().flatten().unwrap_or_default()
}

/// The JSON text of the top-level member `key` of the object `text`, exactly as it was sent, or
/// None. `text` has already parsed as JSON (the relay reads every frame as a Value first), so
/// this only finds where values start and end; every byte it cuts at is ASCII, so every cut is
/// on a character boundary. A key sent twice is answered with its last value, as serde_json
/// reads it, and a key spelled with escapes (`"evidence"`) is read as what it spells.
pub fn raw_member<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let b = text.as_bytes();
    let mut i = skip_ws(b, 0);
    if b.get(i) != Some(&b'{') {
        return None;
    }
    i += 1;
    let mut found = None;
    loop {
        i = skip_ws(b, i);
        match b.get(i)? {
            b'}' => return found,
            b',' => {
                i += 1;
                continue;
            }
            b'"' => {}
            _ => return None,
        }
        let key_end = string_end(b, i)?;
        let name: String = serde_json::from_str(&text[i..key_end]).ok()?;
        i = skip_ws(b, key_end);
        if b.get(i) != Some(&b':') {
            return None;
        }
        i = skip_ws(b, i + 1);
        let value_end = value_end(b, i)?;
        if name == key {
            found = Some(&text[i..value_end]);
        }
        i = value_end;
    }
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while matches!(b.get(i), Some(b' ' | b'\t' | b'\n' | b'\r')) {
        i += 1;
    }
    i
}

/// Where the string starting at `b[i]` (a `"`) ends: the index after its closing quote.
fn string_end(b: &[u8], i: usize) -> Option<usize> {
    let mut j = i + 1;
    loop {
        match b.get(j)? {
            b'\\' => j += 2,
            b'"' => return Some(j + 1),
            _ => j += 1,
        }
    }
}

/// Where the value starting at `b[i]` ends.
fn value_end(b: &[u8], i: usize) -> Option<usize> {
    match b.get(i)? {
        b'"' => string_end(b, i),
        b'{' | b'[' => {
            let (mut depth, mut j) = (0usize, i);
            loop {
                match b.get(j)? {
                    b'"' => {
                        j = string_end(b, j)?;
                        continue;
                    }
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth = depth.checked_sub(1)?;
                        if depth == 0 {
                            return Some(j + 1);
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
        }
        _ => {
            let mut j = i;
            while b.get(j).is_some_and(|c| !matches!(c, b',' | b'}' | b']' | b' ' | b'\t' | b'\n' | b'\r')) {
                j += 1;
            }
            Some(j)
        }
    }
}

// ── Filing a report ────────────────────────────────────────────────────────

/// One evidence item as sent.
enum Item {
    Dm { from: String, to: String, ts: u64, text: String, sig: String },
    Post { from: String, timestamp: u64 },
    GroupText { from: String, ts: u64, text: String },
}

impl Item {
    /// Read evidence item `n` (counting from 1): one of the three kinds with every field.
    fn parse(v: &Value, n: usize) -> Result<Item, String> {
        let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        let num = |k: &str| v.get(k).and_then(Value::as_u64);
        let missing = || format!("evidence item {n} is missing a field.");
        match v.get("kind").and_then(Value::as_str) {
            Some("dm") => Ok(Item::Dm {
                from: s("from").ok_or_else(missing)?,
                to: s("to").ok_or_else(missing)?,
                ts: num("ts").ok_or_else(missing)?,
                text: s("text").ok_or_else(missing)?,
                sig: s("sig").ok_or_else(missing)?,
            }),
            Some("post") => Ok(Item::Post { from: s("from").ok_or_else(missing)?, timestamp: num("timestamp").ok_or_else(missing)? }),
            Some("group_text") => Ok(Item::GroupText {
                from: s("from").ok_or_else(missing)?,
                ts: num("ts").ok_or_else(missing)?,
                text: s("text").ok_or_else(missing)?,
            }),
            _ => Err(format!("evidence item {n} is not a direct message, a post or a group message.")),
        }
    }

    /// Does this item's text carry an encrypted file's marker (any version, anywhere in the text),
    /// and so the key to the file? A direct message's, and a group message's too (a group file
    /// sent the same way would carry its key the same way).
    fn carries_file(&self) -> bool {
        match self {
            Item::Dm { text, .. } | Item::GroupText { text, .. } => text.contains(DM_FILE_MARKER_PREFIX),
            Item::Post { .. } => false,
        }
    }

    /// The item as it is kept, with what the relay could prove of it.
    fn keep(self, state: &RelayState, reporter: &str, target: &str) -> Value {
        match self {
            Item::Dm { from, to, ts, text, sig } => {
                let checked = from == target && to == reporter && verify_dm_inner(target, reporter, ts, &text, &sig);
                serde_json::json!({ "kind": "dm", "from": from, "to": to, "ts": ts, "text": text, "sig": sig, "checked": checked })
            }
            Item::Post { from, timestamp } => {
                let found = if from == target { state.db.post_for_report(&from, timestamp).unwrap_or(None) } else { None };
                match found {
                    Some((text, channel)) => serde_json::json!({
                        "kind": "post", "from": from, "timestamp": timestamp, "text": without_files_or_links(&text),
                        "channel": channel, "checked": true
                    }),
                    None => serde_json::json!({ "kind": "post", "from": from, "timestamp": timestamp, "checked": false }),
                }
            }
            Item::GroupText { from, ts, text } => {
                serde_json::json!({ "kind": "group_text", "from": from, "ts": ts, "text": text, "checked": false })
            }
        }
    }
}

/// A reported post's text as admins are shown it: every uploaded file and every web link
/// replaced by a plain note, so a report never hands anyone a way to open what may be harmful
/// material (2026-10-10, docs/reference/findings/2026-10-10-report-duties-child-abuse-material.md:
/// a post's `/uploads/...` address used to reach admins as text they could click). A direct or
/// group message carrying a file is refused outright (`carries_file`); a post is the server's
/// own copy, not signed by the reporter, so its text can be redacted without breaking any proof.
pub(crate) fn without_files_or_links(text: &str) -> String {
    fn redact(word: &str, out: &mut String) {
        if word.contains("/uploads/") || word.starts_with("uploads/") {
            out.push_str("[a file posted here, not shown]");
        } else if let Some(i) = word.find("://") {
            let host: String = word[i + 3..]
                .chars()
                .take_while(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | ':'))
                .collect();
            if host.is_empty() {
                out.push_str("[a link, not shown]");
            } else {
                out.push_str("[a link to ");
                out.push_str(&host);
                out.push_str(", not shown]");
            }
        } else {
            out.push_str(word);
        }
    }
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    for c in text.chars() {
        if c.is_whitespace() {
            redact(&word, &mut out);
            word.clear();
            out.push(c);
        } else {
            word.push(c);
        }
    }
    redact(&word, &mut out);
    out
}

/// A report that passed every check, ready to keep.
struct Checked {
    target: String,
    context: String,
    reason: String,
    note: String,
    evidence: String,
    evidence_signed: String,
    evidence_hash: String,
    ts: u64,
    sig: String,
}

/// Check `report_v2` from `reporter` (the signed-in socket's key), whose frame text was `text`:
/// the report to keep, or why it is refused (said to the reporter). Cheapest first: the shape,
/// then the limits, then the reporter's signature, and only then the evidence's own signatures.
fn check_report(state: &RelayState, reporter: &str, raw: &Value, text: &str, now: i64) -> Result<Checked, String> {
    let field = |k: &str| raw.get(k).and_then(Value::as_str);
    let target = field("target").unwrap_or("");
    // An identity key is ASCII (hex, or a bot's `bot_` name): anything else names nobody.
    if target.is_empty() || target.len() > TARGET_MAX_LEN || !target.is_ascii() || target.chars().any(char::is_control) {
        return Err("it does not say who it is about.".to_string());
    }
    if target == reporter {
        return Err("you can't report yourself.".to_string());
    }
    let reason = field("reason").unwrap_or("");
    if state.report_reasons.get(reason).is_none() {
        let shown: String = reason.chars().take(40).collect();
        return Err(format!("\"{shown}\" is not one of this server's reasons."));
    }
    let context = field("context").unwrap_or("");
    if !CONTEXTS.contains(&context) {
        return Err("it does not say where it happened (a direct message, a post, a group or a profile).".to_string());
    }
    let note = match raw.get("note") {
        None | Some(Value::Null) => "",
        Some(Value::String(s)) => s.as_str(),
        Some(_) => return Err("its note is not text.".to_string()),
    };
    if note.chars().count() > NOTE_MAX_CHARS {
        return Err(format!("the note is longer than {NOTE_MAX_CHARS} characters."));
    }
    let (Some(ts), Some(sig)) = (raw.get("ts").and_then(Value::as_u64), field("sig")) else {
        return Err("it is not signed.".to_string());
    };
    let evidence_signed = raw_member(text, "evidence").unwrap_or("[]");
    if evidence_signed.len() > EVIDENCE_MAX_BYTES {
        return Err(format!("it carries more than {} KB of evidence.", EVIDENCE_MAX_BYTES / 1024));
    }
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(evidence_signed) else {
        return Err("its evidence is not a list.".to_string());
    };
    if items.len() > EVIDENCE_MAX_ITEMS {
        return Err(format!("it carries more than {EVIDENCE_MAX_ITEMS} pieces of evidence."));
    }
    let items = items.iter().enumerate().map(|(i, v)| Item::parse(v, i + 1)).collect::<Result<Vec<_>, _>>()?;
    if items.iter().any(Item::carries_file) || note.contains(DM_FILE_MARKER_PREFIX) {
        return Err(FILE_IN_EVIDENCE.to_string());
    }

    let filed = state.db.reports_filed_since(reporter, now - 3_600_000).map_err(|e| {
        tracing::error!("Reports: could not count a reporter's reports: {e}");
        "the server could not check it. Try again.".to_string()
    })?;
    if filed >= REPORTS_PER_HOUR {
        return Err(format!("you have sent {REPORTS_PER_HOUR} reports in the last hour, this server's limit. Please wait before sending another."));
    }
    if state.db.reported_since(reporter, target, now - SAME_TARGET_COOLDOWN_MS).unwrap_or(false) {
        return Err("you already reported this person in the last 24 hours, and the admins have that report.".to_string());
    }

    let evidence_hash = report_evidence_hash(evidence_signed);
    if !verify_report_sig(reporter, target, reason, &evidence_hash, ts, sig) {
        return Err("its signature did not check out. Try again from the app.".to_string());
    }
    let kept: Vec<Value> = items.into_iter().map(|item| item.keep(state, reporter, target)).collect();
    Ok(Checked {
        target: target.to_string(),
        context: context.to_string(),
        reason: reason.to_string(),
        note: note.to_string(),
        evidence: Value::Array(kept).to_string(),
        evidence_signed: evidence_signed.to_string(),
        evidence_hash,
        ts,
        sig: sig.to_string(),
    })
}

/// `report_v2` from the signed-in `reporter`, whose frame text was `text`: check it, keep it,
/// answer `report_received {id}`, and tell the admins and moderators online that it came.
pub async fn handle_report(state: &Arc<RelayState>, reporter: &str, raw: &Value, text: &str) {
    let now = now_ms();
    let c = match check_report(state, reporter, raw, text, now) {
        Ok(c) => c,
        Err(why) if why == FILE_IN_EVIDENCE => return tell(state, reporter, why),
        Err(why) => return tell(state, reporter, format!("Your report was not sent: {why}")),
    };
    let new = NewReport {
        reporter_key: reporter,
        target_key: &c.target,
        context: &c.context,
        reason: &c.reason,
        note: &c.note,
        evidence: &c.evidence,
        evidence_signed: &c.evidence_signed,
        evidence_hash: &c.evidence_hash,
        report_ts: c.ts,
        report_sig: &c.sig,
    };
    let id = match state.db.report_add(&new, now) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Reports: could not keep a report: {e}");
            return tell(state, reporter, "Your report was not sent: the server could not keep it. Try again.".to_string());
        }
    };
    let _ = state.broadcast_tx.send(RelayMessage::ReportReceived { to: reporter.to_string(), id });
    let about = match name_of(state, &c.target) {
        n if n.is_empty() => "someone".to_string(),
        n => n,
    };
    let notice = format!("New report #{id} about {about}: {}. Open Reports to review it.", state.report_reasons.label(&c.reason));
    let staff: Vec<String> = state.peers.read().await.keys().filter(|k| is_staff(&role_of(state, k))).cloned().collect();
    for key in staff {
        tell(state, &key, notice.clone());
    }
}

// ── Reviewing ──────────────────────────────────────────────────────────────

/// One report as the review is sent it. Admins see who reported (`reporter`, the key, empty once
/// the reporter erased their account, and `reporter_name`); a moderator gets only
/// `reporter_name: "a member"`, and a direct message's `to` and signature are left out of the
/// evidence they are sent, since either names the reporter.
pub fn item_json(state: &RelayState, r: &ReportRow, for_admin: bool) -> Value {
    let mut evidence: Vec<Value> = serde_json::from_str(&r.evidence).unwrap_or_default();
    if !for_admin {
        for item in evidence.iter_mut() {
            if let Some(obj) = item.as_object_mut() {
                obj.remove("to");
                obj.remove("sig");
            }
        }
    }
    let mut v = serde_json::json!({
        "id": r.id,
        "target": r.target_key,
        "target_name": name_of(state, &r.target_key),
        "context": r.context,
        "reason": r.reason,
        "reason_label": state.report_reasons.label(&r.reason),
        "note": r.note,
        "evidence": evidence,
        "created_at": r.created_at,
        "state": r.state,
        "decision": r.decision,
        "decision_note": r.decision_note,
        "decided_by": r.decided_by,
        "decided_by_name": r.decided_by.as_deref().map(|k| name_of(state, k)),
        "decided_at": r.decided_at,
        "reporter_name": A_MEMBER,
    });
    if for_admin {
        let name = name_of(state, &r.reporter_key);
        let shown = if r.reporter_key.is_empty() {
            AN_ERASED_ACCOUNT.to_string()
        } else if name.is_empty() {
            A_MEMBER.to_string()
        } else {
            name
        };
        v["reporter"] = Value::String(r.reporter_key.clone());
        v["reporter_name"] = Value::String(shown);
    }
    v
}

/// `reports_list {state}` from `me`: the open or the decided reports, newest first, to admins and
/// moderators only.
pub fn handle_list(state: &Arc<RelayState>, me: &str, raw: &Value) {
    let role = role_of(state, me);
    if !is_staff(&role) {
        return tell(state, me, "Only admins and moderators can see reports.".to_string());
    }
    let want = raw.get("state").and_then(Value::as_str).unwrap_or("open");
    if want != "open" && want != "decided" {
        return tell(state, me, "Reports are listed as \"open\" or \"decided\".".to_string());
    }
    let rows = state.db.reports_in_state(want, LIST_MAX).unwrap_or_else(|e| {
        tracing::error!("Reports: could not read the {want} reports: {e}");
        Vec::new()
    });
    let items = rows.iter().map(|r| item_json(state, r, role == "admin")).collect();
    let _ = state.broadcast_tx.send(RelayMessage::Reports { to: me.to_string(), state: want.to_string(), items });
}

/// The public posts of `target` a report's evidence names: (author, timestamp).
fn posts_of(r: &ReportRow) -> Vec<(String, u64)> {
    let evidence: Vec<Value> = serde_json::from_str(&r.evidence).unwrap_or_default();
    evidence
        .iter()
        .filter(|i| i.get("kind").and_then(Value::as_str) == Some("post"))
        .filter(|i| i.get("from").and_then(Value::as_str) == Some(r.target_key.as_str()))
        .filter_map(|i| i.get("timestamp").and_then(Value::as_u64))
        .map(|ts| (r.target_key.clone(), ts))
        .collect()
}

/// `report_decide {id, decision, note}` from `me`, an admin or moderator: carry the decision out
/// through the moderation path, then record it on the report (reviewer, decision, note, time).
/// Refused, and nothing recorded, when the moderation rules refuse the action (a moderator
/// banning, or acting on an admin), when the report is already decided, when a moderator decides
/// a report about themselves, and when `delete_post` finds no post of theirs in the report.
pub async fn handle_decide(state: &Arc<RelayState>, me: &str, raw: &Value) {
    let role = role_of(state, me);
    if !is_staff(&role) {
        return tell(state, me, "Only admins and moderators can decide reports.".to_string());
    }
    let Some(id) = raw.get("id").and_then(Value::as_i64) else {
        return tell(state, me, "Which report? The decision names none.".to_string());
    };
    let decision = raw.get("decision").and_then(Value::as_str).unwrap_or("");
    if !DECISIONS.contains(&decision) {
        return tell(state, me, format!("Report #{id} was not decided: a decision is one of {}.", DECISIONS.join(", ")));
    }
    let note = raw.get("note").and_then(Value::as_str).unwrap_or("");
    if note.chars().count() > NOTE_MAX_CHARS {
        return tell(state, me, format!("Report #{id} was not decided: the note is longer than {NOTE_MAX_CHARS} characters."));
    }
    let report = match state.db.report_by_id(id) {
        Ok(Some(r)) => r,
        Ok(None) => return tell(state, me, format!("There is no report #{id}.")),
        Err(e) => {
            tracing::error!("Reports: could not read report #{id}: {e}");
            return tell(state, me, format!("Report #{id} was not decided: the server could not read it."));
        }
    };
    if report.state != "open" {
        return tell(state, me, format!("Report #{id} was already decided ({}).", report.decision.as_deref().unwrap_or("")));
    }
    let target = report.target_key.clone();
    if target == me && role != "admin" {
        return tell(state, me, format!("Report #{id} is about you, so an admin decides it."));
    }
    match decision {
        "warn" | "mute" | "kick" | "ban" => {
            let name = name_of(state, &target);
            if let Some(why) = mod_action_refusal(state, me, decision, &target, &name) {
                return tell(state, me, format!("Report #{id} was not decided: {why}"));
            }
            handle_mod_action(state, me, decision, &target, &name).await;
        }
        "delete_post" => {
            if let Some(why) = mod_action_refusal(state, me, decision, &target, "") {
                return tell(state, me, format!("Report #{id} was not decided: {why}"));
            }
            let posts = posts_of(&report);
            if posts.is_empty() {
                return tell(state, me, format!("Report #{id} was not decided: it has no post of theirs to delete."));
            }
            for (from, timestamp) in posts {
                match state.db.delete_message(&from, timestamp) {
                    Ok(true) => {
                        let _ = state.broadcast_tx.send(RelayMessage::Delete { from, timestamp });
                    }
                    Ok(false) => {} // already gone (its author deleted it, or it expired)
                    Err(e) => tracing::error!("Reports: could not delete a reported post: {e}"),
                }
            }
        }
        _ => {} // dismiss: nothing to carry out
    }
    match state.db.report_record_decision(id, me, decision, note, now_ms()) {
        Ok(true) => tell(state, me, format!("Report #{id} decided: {decision}.")),
        Ok(false) => tell(state, me, format!("Report #{id} was already decided.")),
        Err(e) => {
            tracing::error!("Reports: could not record the decision on report #{id}: {e}");
            tell(state, me, format!("Report #{id}: the {decision} was carried out, but the server could not record it on the report."));
        }
    }
}

/// The report messages relay.rs hands here: `report_v2` (whose frame `text` is needed for the
/// evidence's exact text), `reports_list` and `report_decide`.
pub async fn handle(state: &Arc<RelayState>, my_key: &str, kind: &str, raw: &Value, text: &str) {
    match kind {
        "report_v2" => handle_report(state, my_key, raw, text).await,
        "reports_list" => handle_list(state, my_key, raw),
        "report_decide" => handle_decide(state, my_key, raw).await,
        _ => {}
    }
}

// ── The slash commands ─────────────────────────────────────────────────────

/// How long ago `then_ms` was, in a few characters.
fn ago(now: i64, then_ms: i64) -> String {
    let s = ((now - then_ms) / 1000).max(0);
    match s {
        s if s < 60 => format!("{s}s ago"),
        s if s < 3_600 => format!("{}m ago", s / 60),
        s if s < 86_400 => format!("{}h ago", s / 3_600),
        s => format!("{}d ago", s / 86_400),
    }
}

/// `/report`, `/reports` and `/reports-clear`, typed in chat by `me`. `/report` says how reports
/// are made now; `/reports` lists the latest reports (admins and moderators, moderators seeing
/// "a member" for whoever reported); `/reports-clear` deletes the decided reports (admins; open
/// reports stay until someone decides them).
pub async fn handle_slash(state: &Arc<RelayState>, me: &str, cmd: &str) {
    let role = role_of(state, me);
    match cmd {
        "/report" => tell(state, me, REPORT_HOW.to_string()),
        "/reports" => {
            if !is_staff(&role) {
                return tell(state, me, "Only admins and moderators can see reports.".to_string());
            }
            let rows = match state.db.reports_latest(SLASH_LIST_MAX) {
                Ok(rows) => rows,
                Err(e) => {
                    tracing::error!("Reports: could not read the latest reports: {e}");
                    return tell(state, me, "The reports could not be read.".to_string());
                }
            };
            if rows.is_empty() {
                return tell(state, me, "No reports.".to_string());
            }
            let now = now_ms();
            let mut lines = vec!["Recent reports (open them in Reports to see the evidence):".to_string()];
            for r in &rows {
                let item = item_json(state, r, role == "admin");
                let about = match name_of(state, &r.target_key) {
                    n if n.is_empty() => r.target_key.chars().take(8).collect::<String>(),
                    n => n,
                };
                let status = match &r.decision {
                    Some(d) => format!("decided: {d}"),
                    None => "open".to_string(),
                };
                lines.push(format!(
                    "  #{} | {status} | {} | about {about} | from {} | {}",
                    r.id,
                    state.report_reasons.label(&r.reason),
                    item["reporter_name"].as_str().unwrap_or(A_MEMBER),
                    ago(now, r.created_at)
                ));
            }
            tell(state, me, lines.join("\n"));
        }
        "/reports-clear" => {
            if role != "admin" {
                return tell(state, me, "Only admins can clear reports.".to_string());
            }
            match state.db.reports_clear_decided() {
                Ok(n) => tell(state, me, format!("Cleared {n} decided report(s). Open reports stay until someone decides them.")),
                Err(e) => {
                    tracing::error!("Reports: could not clear the decided reports: {e}");
                    tell(state, me, "The reports could not be cleared.".to_string());
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
#[path = "reports_tests.rs"]
mod tests;
