//! Reports the admins can check (step D of docs/design/blocking-and-safe-mode.md, section 10e,
//! 2026-10-09): the native client's rules, with no socket and no GUI so each one is unit tested.
//! The app-state glue (opening the dialog, sending, the relay's answers) is src/engine/report.rs;
//! the Report dialog is src/gui/pages/chat/report_dialog.rs and the admins' Reports list is
//! src/gui/pages/server_settings/reports.rs.
//!
//! WHAT A REPORT IS (10e, the names and shapes fixed so the relay and both clients agree):
//! `{"type":"report_v2","target","context","reason","note","evidence":[...],"ts","sig"}`, where
//! `sig` is the reporter's Dilithium3 signature over
//! `"hum/report/v1\n{reporter}\n{target}\n{reason}\n{evidence_hash}\n{ts}"` and `evidence_hash`
//! is the lowercase hex BLAKE3 of the `evidence` array's JSON text exactly as it is sent. This
//! file builds that text once, hashes it, and writes the same bytes into the frame, so the hash
//! the relay computes over what it received is the one that was signed.
//!
//! Choices this file makes that 10e left open (said again in the step D commit):
//! - `sig` is standard base64, like every other `hum/.../vN` signature in the protocol (the DM
//!   inner payload, the identify answer, the friendship pass); hex is used only by the older
//!   `content\ntimestamp` chat signature.
//! - `ts` is milliseconds since 1970, like a DM's `ts` and a chat message's `timestamp`.
//! - Each evidence item's keys are written in alphabetical order. The relay hashes the text as
//!   received, so any order works; alphabetical also equals what serde_json re-serialises to, so
//!   the hash still matches if a server ever hashes a re-serialised copy instead.
//!
//! The evidence a DM report carries is the verified inner payload the DM store keeps for each
//! message (`StoredDm`, with its `sig` since step D): from the target, to us. The relay checks
//! the signature over the DM v2 preimage (`dm_pq::sig_preimage`) against the target's key, so
//! nobody can forge an item or pin someone else's words on the target. A message kept before
//! step D (no signature) or carrying a file (`carries_file`) is never offered.

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::Deserialize;
use serde_json::{Map, Value};

use super::dm_store::StoredDm;

/// The signature domain of a report. The relay rebuilds this preimage byte for byte; the web
/// client must use the identical string.
pub const REPORT_DOMAIN: &str = "hum/report/v1";

/// The reasons file (under `data/`), created by the relay half of step D and read by both
/// clients and the relay, so a reason id means the same everywhere.
pub const REASONS_FILE: &str = "safety/report_reasons.json";

/// At most this many evidence items in one report (10e).
pub const MAX_EVIDENCE_ITEMS: usize = 20;
/// At most this many bytes of evidence JSON in one report (10e: 64 KB).
pub const MAX_EVIDENCE_BYTES: usize = 64 * 1024;
/// The note's limit, in characters (10e).
pub const MAX_NOTE_CHARS: usize = 500;
/// How many of the person's messages the evidence picker lists, newest first. A display bound
/// only: a long conversation stays scrollable, and only 20 can be sent anyway.
pub const PICKER_LIMIT: usize = 200;

/// One reason from `data/safety/report_reasons.json`: its id (what the relay checks), the label
/// the dialog shows, and the help text shown once it is chosen.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ReportReason {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub help: String,
}

/// Read the reasons file: the ordered list itself, or an object holding it as `reasons`. An
/// entry with no id is skipped; a file with no usable entry is an error, so the dialog says the
/// list could not be loaded rather than offering nothing.
pub fn parse_reasons(bytes: &[u8]) -> Result<Vec<ReportReason>, String> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum File {
        List(Vec<ReportReason>),
        Wrapped { reasons: Vec<ReportReason> },
    }
    let file: File = serde_json::from_slice(bytes).map_err(|e| format!("report reasons: {e}"))?;
    let list = match file {
        File::List(list) => list,
        File::Wrapped { reasons } => reasons,
    };
    let list: Vec<ReportReason> = list.into_iter().filter(|r| !r.id.trim().is_empty()).collect();
    if list.is_empty() {
        return Err("report reasons: the list is empty".to_string());
    }
    Ok(list)
}

/// Load the reasons from the data folder (`data/safety/report_reasons.json`).
pub fn load_reasons(data_dir: &std::path::Path) -> Result<Vec<ReportReason>, String> {
    crate::embedded_data::load_data_or_embedded(data_dir, REASONS_FILE, parse_reasons)
}

/// Where the report was made from (10e's `context`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportContext {
    /// A direct message conversation: evidence is the person's signed messages.
    Dm,
    /// A public post in a channel: evidence is the post (author key and time).
    Post,
    /// A message in a P2P group: evidence is its text, which cannot be proven to the server.
    Group,
    /// A person's entry in the member list or their profile: no evidence.
    Profile,
}

impl ReportContext {
    pub fn wire(self) -> &'static str {
        match self {
            ReportContext::Dm => "dm",
            ReportContext::Post => "post",
            ReportContext::Group => "group",
            ReportContext::Profile => "profile",
        }
    }
}

/// One evidence item as it is sent (10e).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evidence {
    /// A DM's verified inner payload, as the DM store keeps it: the relay checks `sig` over the
    /// DM v2 preimage against the target's key, with `from` = target and `to` = reporter.
    Dm { from: String, to: String, ts: u64, text: String, sig: String },
    /// A public post: the relay looks it up by author key and timestamp and keeps its own text.
    Post { from: String, timestamp: u64 },
    /// A P2P group message: kept unproven, and labelled so.
    GroupText { from: String, ts: u64, text: String },
}

impl Evidence {
    /// The item as JSON with its keys in alphabetical order (see the file header). serde_json's map
    /// is sorted in this build (its `preserve_order` feature is off); inserting alphabetically
    /// keeps the order the same if that feature is ever switched on.
    pub fn to_value(&self) -> Value {
        let mut m = Map::new();
        match self {
            Evidence::Dm { from, to, ts, text, sig } => {
                m.insert("from".into(), Value::from(from.as_str()));
                m.insert("kind".into(), Value::from("dm"));
                m.insert("sig".into(), Value::from(sig.as_str()));
                m.insert("text".into(), Value::from(text.as_str()));
                m.insert("to".into(), Value::from(to.as_str()));
                m.insert("ts".into(), Value::from(*ts));
            }
            Evidence::Post { from, timestamp } => {
                m.insert("from".into(), Value::from(from.as_str()));
                m.insert("kind".into(), Value::from("post"));
                m.insert("timestamp".into(), Value::from(*timestamp));
            }
            Evidence::GroupText { from, ts, text } => {
                m.insert("from".into(), Value::from(from.as_str()));
                m.insert("kind".into(), Value::from("group_text"));
                m.insert("text".into(), Value::from(text.as_str()));
                m.insert("ts".into(), Value::from(*ts));
            }
        }
        Value::Object(m)
    }
}

/// The `evidence` array's compact JSON text: exactly what is hashed AND what goes in the frame.
pub fn evidence_json(items: &[Evidence]) -> String {
    Value::Array(items.iter().map(Evidence::to_value).collect()).to_string()
}

/// Lowercase hex BLAKE3 of the evidence JSON text.
pub fn evidence_hash(evidence_json: &str) -> String {
    blake3::hash(evidence_json.as_bytes()).to_hex().to_string()
}

/// The words the reporter signs (10e). `reporter` is the signed-in socket's key.
pub fn report_preimage(reporter: &str, target: &str, reason: &str, evidence_hash: &str, ts: u64) -> String {
    format!("{REPORT_DOMAIN}\n{reporter}\n{target}\n{reason}\n{evidence_hash}\n{ts}")
}

/// What the dialog hands the builder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportDraft {
    pub target: String,
    pub context: ReportContext,
    pub reason: String,
    pub note: String,
    pub evidence: Vec<Evidence>,
    pub ts: u64,
}

/// Why a draft cannot be sent; each is a sentence the dialog shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftProblem {
    NoTarget,
    SelfReport,
    NoReason,
    NoteTooLong,
    TooManyItems,
    EvidenceTooLarge,
    Locked,
}

impl DraftProblem {
    pub fn sentence(self) -> &'static str {
        match self {
            DraftProblem::NoTarget => "There is no one to report here.",
            DraftProblem::SelfReport => "You cannot report yourself.",
            DraftProblem::NoReason => "Choose a reason first.",
            DraftProblem::NoteTooLong => "The note is too long: 500 characters at most.",
            DraftProblem::TooManyItems => "Too many messages ticked: 20 at most.",
            DraftProblem::EvidenceTooLarge => "The ticked messages are too long to send together (64 KB at most). Untick some.",
            DraftProblem::Locked => "Unlock your identity to send a report: it is signed with your key.",
        }
    }
}

/// Check a draft against 10e's limits and return its evidence JSON text.
pub fn check_draft(draft: &ReportDraft, reporter: &str) -> Result<String, DraftProblem> {
    if draft.target.trim().is_empty() {
        return Err(DraftProblem::NoTarget);
    }
    if draft.target == reporter {
        return Err(DraftProblem::SelfReport);
    }
    if draft.reason.trim().is_empty() {
        return Err(DraftProblem::NoReason);
    }
    if draft.note.chars().count() > MAX_NOTE_CHARS {
        return Err(DraftProblem::NoteTooLong);
    }
    if draft.evidence.len() > MAX_EVIDENCE_ITEMS {
        return Err(DraftProblem::TooManyItems);
    }
    let json = evidence_json(&draft.evidence);
    if json.len() > MAX_EVIDENCE_BYTES {
        return Err(DraftProblem::EvidenceTooLarge);
    }
    Ok(json)
}

/// Build the `report_v2` frame, signed with the identity's Dilithium3 key re-derived from
/// `seed` (the same derivation as the chat and DM signatures). The frame is written by hand
/// around the evidence text so the bytes the relay hashes are the bytes signed here.
pub fn build_report_frame(seed: &[u8], reporter: &str, draft: &ReportDraft) -> Result<String, DraftProblem> {
    let evidence = check_draft(draft, reporter)?;
    if seed.is_empty() || reporter.is_empty() {
        return Err(DraftProblem::Locked);
    }
    let hash = evidence_hash(&evidence);
    let preimage = report_preimage(reporter, &draft.target, &draft.reason, &hash, draft.ts);
    let sig = B64.encode(crate::net::identity::pq_sign_raw(seed, preimage.as_bytes()));
    let s = |v: &str| Value::from(v).to_string();
    Ok(format!(
        "{{\"type\":\"report_v2\",\"target\":{},\"context\":{},\"reason\":{},\"note\":{},\"evidence\":{},\"ts\":{},\"sig\":{}}}",
        s(&draft.target),
        s(draft.context.wire()),
        s(&draft.reason),
        s(&draft.note),
        evidence,
        draft.ts,
        s(&sig),
    ))
}

// ── The evidence picker (a DM report) ───────────────────────────────────────────────────────

/// One of the person's messages offered as evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmCandidate {
    /// The item that is sent when ticked.
    pub item: Evidence,
    /// The message's own (sender-claimed) time, for the list.
    pub ts: u64,
    /// The message text, for the list.
    pub text: String,
    pub ticked: bool,
}

/// What the dialog says about messages with files, which are never offered (`carries_file`).
pub const FILES_NOT_INCLUDED: &str = "Messages with files cannot be included in a report.";

/// Does this message carry an encrypted file? Its text then holds the key that opens the file
/// (`dm_pq::build_file_marker`), and the signature covers the whole text, so the key cannot be
/// taken out. A report never hands an admin the means to open a file that could be abuse
/// imagery: viewing or passing that on is itself a crime, and it belongs with the official
/// hotlines. So such a message is never evidence, here or on the relay, which refuses it too.
/// Any version of the marker anywhere in the text counts (`FILE_MARKER_ANY`), the same test the
/// web client makes (`reportTextHasFile` in web/shared/report.js).
pub fn carries_file(text: &str) -> bool {
    text.contains(FILE_MARKER_ANY)
}

/// The start every version of the encrypted-file marker shares (`dm_pq::FILE_MARKER` is v1).
pub const FILE_MARKER_ANY: &str = "[[hum:file:";

/// THE picker's choice: the target's messages to us in this conversation (never ours, never a
/// third person's), newest first, at most `PICKER_LIMIT`. Never offered: a message kept before
/// step D, which has no signature in the store and so proves nothing, and a message with a file
/// (`carries_file`). One is ticked: the message the Report was pressed on when it is one of
/// those offered (`pressed_ts`), otherwise the most recent.
pub fn dm_candidates(conversation: &[StoredDm], target: &str, me: &str, pressed_ts: Option<u64>) -> Vec<DmCandidate> {
    if target.is_empty() || target == me {
        return Vec::new();
    }
    let mut theirs: Vec<&StoredDm> = conversation
        .iter()
        .filter(|m| m.from == target && m.to == me)
        .filter(|m| !m.sig.is_empty())
        .filter(|m| !carries_file(&m.text))
        .collect();
    theirs.sort_by(|a, b| b.ts.cmp(&a.ts)); // newest first (stable among equal times)
    theirs.truncate(PICKER_LIMIT);
    let mut out: Vec<DmCandidate> = theirs
        .into_iter()
        .map(|m| DmCandidate {
            item: Evidence::Dm { from: m.from.clone(), to: m.to.clone(), ts: m.ts, text: m.text.clone(), sig: m.sig.clone() },
            ts: m.ts,
            text: m.text.clone(),
            ticked: false,
        })
        .collect();
    let pick = pressed_ts.and_then(|ts| out.iter().position(|c| c.ts == ts)).unwrap_or(0);
    if let Some(c) = out.get_mut(pick) {
        c.ticked = true;
    }
    out
}

/// The ticked messages as evidence, oldest first so an admin reads them in order.
pub fn chosen(candidates: &[DmCandidate]) -> Vec<Evidence> {
    let mut picked: Vec<&DmCandidate> = candidates.iter().filter(|c| c.ticked).collect();
    picked.sort_by_key(|c| c.ts);
    picked.into_iter().map(|c| c.item.clone()).collect()
}

// ── The admins' side: `reports` ─────────────────────────────────────────────────────────────

/// The decisions an admin or moderator can make (10e), carried out by the relay through its
/// moderation path, with that path's rules. Fixed by the protocol, like the reach audiences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Dismiss,
    Warn,
    Mute,
    Kick,
    Ban,
    DeletePost,
}

impl Decision {
    pub const ALL: [Decision; 6] =
        [Decision::Dismiss, Decision::Warn, Decision::Mute, Decision::Kick, Decision::Ban, Decision::DeletePost];

    pub fn wire(self) -> &'static str {
        match self {
            Decision::Dismiss => "dismiss",
            Decision::Warn => "warn",
            Decision::Mute => "mute",
            Decision::Kick => "kick",
            Decision::Ban => "ban",
            Decision::DeletePost => "delete_post",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Decision::Dismiss => "Dismiss",
            Decision::Warn => "Warn",
            Decision::Mute => "Mute",
            Decision::Kick => "Kick",
            Decision::Ban => "Ban",
            Decision::DeletePost => "Delete post",
        }
    }

    pub fn tip(self) -> &'static str {
        match self {
            Decision::Dismiss => "Close the report with no action against them.",
            Decision::Warn => "Send them the server's warning, a fixed sentence that names nobody.",
            Decision::Mute => "Stop them posting and sending direct messages here until unmuted.",
            Decision::Kick => "Disconnect them now. They can come back.",
            Decision::Ban => "Ban their key from this server.",
            Decision::DeletePost => "Delete their posts named in this report's evidence.",
        }
    }

    /// What a decided report says it was, from the wire word (unknown words shown as they are).
    pub fn label_of(wire: &str) -> String {
        Decision::ALL.iter().find(|d| d.wire() == wire).map(|d| d.label().to_string()).unwrap_or_else(|| wire.to_string())
    }
}

/// The `report_decide` frame (admins and mods only; the relay checks).
pub fn decide_frame(id: &Value, decision: Decision, note: &str) -> String {
    serde_json::json!({ "type": "report_decide", "id": id, "decision": decision.wire(), "note": note }).to_string()
}

/// The `reports_list` frame: `open` or `decided`.
pub fn list_frame(decided: bool) -> String {
    serde_json::json!({ "type": "reports_list", "state": if decided { "decided" } else { "open" } }).to_string()
}

/// One evidence item as the relay lists it, with its `checked` mark.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListedEvidence {
    pub kind: String,
    pub from: String,
    pub to: String,
    pub ts: u64,
    pub text: String,
    pub checked: bool,
}

/// One report as the relay lists it (the relay half of step D fixed these names). `id` is kept
/// as the JSON value it arrived as, so a decision names it the same way back. `reporter` (the
/// key) reaches admins only, empty once the reporter erased their account; `reporter_name` is
/// their name, "an erased account", or for a moderator always "a member". A moderator's DM
/// items also arrive without `to` and `sig` (they would identify the reporter), so the badge
/// comes from `checked`, never from checking again here.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ListedReport {
    pub id: Value,
    pub target: String,
    pub target_name: String,
    pub context: String,
    pub reason: String,
    /// The relay's label for `reason` (from the same data file).
    pub reason_label: String,
    pub note: String,
    pub evidence: Vec<ListedEvidence>,
    pub created_at: u64,
    pub state: String,
    pub decision: String,
    pub decision_note: String,
    pub decided_by: String,
    pub decided_by_name: String,
    pub decided_at: u64,
    pub reporter: String,
    pub reporter_name: String,
}

impl ListedReport {
    /// The id as text, for keys and the list.
    pub fn id_text(&self) -> String {
        match &self.id {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        }
    }

    pub fn is_decided(&self) -> bool {
        !self.decision.is_empty() || self.state == "decided"
    }

    /// Whether the report names a post a decision could delete.
    pub fn has_post(&self) -> bool {
        self.context == "post" || self.evidence.iter().any(|e| e.kind == "post")
    }
}

/// A string field, or empty (a missing field and a JSON null both read as empty).
fn str_at(v: &Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

/// A whole-number field, or 0.
fn u64_at(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(|x| x.as_u64()).unwrap_or(0)
}

/// Read a `reports` frame's `items` (`{"type":"reports","state":"open"|"decided","items":[...]}`).
/// An evidence item's time is `ts`, or `timestamp` for a post.
pub fn parse_reports(frame: &Value) -> Vec<ListedReport> {
    let Some(items) = frame.get("items").and_then(|v| v.as_array()) else { return Vec::new() };
    items
        .iter()
        .filter(|it| it.get("id").is_some_and(|id| !id.is_null()))
        .map(|it| {
            let evidence = it
                .get("evidence")
                .and_then(|e| e.as_array())
                .map(|list| {
                    list.iter()
                        .map(|e| ListedEvidence {
                            kind: str_at(e, "kind"),
                            from: str_at(e, "from"),
                            to: str_at(e, "to"),
                            ts: e.get("ts").or_else(|| e.get("timestamp")).and_then(|x| x.as_u64()).unwrap_or(0),
                            text: str_at(e, "text"),
                            checked: e.get("checked").and_then(|c| c.as_bool()).unwrap_or(false),
                        })
                        .collect()
                })
                .unwrap_or_default();
            ListedReport {
                id: it.get("id").cloned().unwrap_or(Value::Null),
                target: str_at(it, "target"),
                target_name: str_at(it, "target_name"),
                context: str_at(it, "context"),
                reason: str_at(it, "reason"),
                reason_label: str_at(it, "reason_label"),
                note: str_at(it, "note"),
                evidence,
                created_at: u64_at(it, "created_at"),
                state: str_at(it, "state"),
                decision: str_at(it, "decision"),
                decision_note: str_at(it, "decision_note"),
                decided_by: str_at(it, "decided_by"),
                decided_by_name: str_at(it, "decided_by_name"),
                decided_at: u64_at(it, "decided_at"),
                reporter: str_at(it, "reporter"),
                reporter_name: str_at(it, "reporter_name"),
            }
        })
        .collect()
}

/// A time the relay sent, as milliseconds: a value too small to be milliseconds since 1970 is
/// read as seconds (10e does not say which the list carries).
pub fn as_millis(t: u64) -> u64 {
    if t > 0 && t < 100_000_000_000 { t * 1000 } else { t }
}

/// The badge on one listed evidence item (10e): a proven DM says who sent it to whom; a post
/// the server found says who posted it; anything else is Not proven. `name` is the sender's
/// name as the page knows it.
pub fn badge(e: &ListedEvidence, name: &str) -> String {
    match (e.checked, e.kind.as_str()) {
        (true, "dm") => format!("Signature checked: sent by {name} to the reporter"),
        (true, "post") => format!("Found on this server: posted by {name}"),
        (true, _) => "Checked by the server".to_string(),
        (false, _) => "Not proven".to_string(),
    }
}

/// The sentence the Reports list shows about what a checked signature does not prove (10e).
pub const CHECKED_DOES_NOT_PROVE: &str = "A checked signature proves the person wrote exactly those words and sent them to the reporter. \
     It does not prove when: the time is the sender's own clock. And the reporter chose which messages to include, so \
     context may be missing.";

// ── State the GUI keeps ─────────────────────────────────────────────────────────────────────

/// The Report dialog while it is open.
#[derive(Debug, Clone, Default)]
pub struct ReportDialog {
    pub target: String,
    pub target_name: String,
    pub context: Option<ReportContext>,
    /// The chosen reason's id (empty until one is chosen).
    pub reason: String,
    pub note: String,
    /// A DM report's picker.
    pub candidates: Vec<DmCandidate>,
    /// A post's or group message's one item, sent as it is.
    pub fixed: Option<Evidence>,
    /// The text of that post or group message, shown in the dialog.
    pub fixed_text: String,
    /// "Also block them": ticked by default for a DM report (10e).
    pub also_block: bool,
    /// Why the last Send did not go.
    pub problem: String,
}

/// Step D's part of the app state (`GuiState::reports`).
#[derive(Debug, Default)]
pub struct ReportUi {
    /// The reasons, loaded the first time a dialog opens.
    pub reasons: Vec<ReportReason>,
    /// Why they could not be loaded, if they could not.
    pub reasons_error: Option<String>,
    pub dialog: Option<ReportDialog>,
    /// The Reports list (admins and mods): showing decided reports rather than open ones.
    pub show_decided: bool,
    /// The list as the server last sent it, and whether it holds decided reports.
    pub list: Vec<ListedReport>,
    pub list_is_decided: bool,
    /// Whether the last `reports_list` asked for decided reports (the answer may not say).
    pub asked_decided: bool,
    /// A `reports_list` has been sent for the list shown; cleared on a new socket or server so
    /// the page asks again.
    pub requested: bool,
    /// The note an admin is writing for each open report's decision, by report id.
    pub decide_notes: std::collections::HashMap<String, String>,
    /// One line about the list (a decision sent, not connected).
    pub status: String,
}

impl ReportUi {
    /// A new socket or another server: the list is that server's, so it is asked for again.
    pub fn forget_server(&mut self) {
        self.list.clear();
        self.requested = false;
        self.decide_notes.clear();
        self.status.clear();
    }
}

/// Every rule above that needs no socket. Each test was seen red once on purpose, recorded at
/// the test.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::core::pq_crypto::{derive_dilithium_seed, verify_dilithium, DilithiumKeypair};

    /// The evidence the pins below are built from: one DM item and one post, values chosen to
    /// be short and readable.
    fn pinned_items() -> Vec<Evidence> {
        vec![
            Evidence::Dm { from: "CCDD".into(), to: "AABB".into(), ts: 1_791_503_000_000, text: "go away".into(), sig: "c2ln".into() },
            Evidence::Post { from: "CCDD".into(), timestamp: 1_791_503_500_000 },
        ]
    }

    /// The evidence array's compact JSON, keys alphabetical within each item.
    const PINNED_EVIDENCE_JSON: &str = r#"[{"from":"CCDD","kind":"dm","sig":"c2ln","text":"go away","to":"AABB","ts":1791503000000},{"from":"CCDD","kind":"post","timestamp":1791503500000}]"#;
    /// BLAKE3 of PINNED_EVIDENCE_JSON, lowercase hex.
    const PINNED_EVIDENCE_HASH: &str = "292959951358502afa69ae5c8d25d7b099a419814ae552c83edceff50de4e35d";
    /// THE report preimage (10e), copied from the relay's pin
    /// (`report_preimage_and_evidence_hash_are_pinned` in src/relay/core/pq_crypto.rs, which the
    /// web client's Node test also reads): the relay team pins the same string, so if either
    /// side changes these words its pin goes red. The hash in it is BLAKE3's published value
    /// for no input.
    const PINNED_REPORT_PREIMAGE: &str =
        "hum/report/v1\nAABB\nCCDD\nharassment\naf1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262\n1760000000000";
    /// BLAKE3 of no input (the relay's pin uses it as the evidence hash in the preimage above).
    const BLAKE3_OF_NOTHING: &str = "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262";

    fn identity(n: u8) -> (Vec<u8>, String) {
        let seed = vec![n; 32];
        (seed.clone(), hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed)).public_key()))
    }

    fn stored(from: &str, to: &str, ts: u64, text: &str, sig: &str) -> StoredDm {
        StoredDm { from: from.into(), to: to.into(), ts, text: text.into(), dedupe: format!("{from}{ts}"), sig: sig.into() }
    }

    /// Seen red 2026-10-09 three ways: the hash pin was first run against a placeholder and
    /// failed with the real value, which is what is pinned; with the post item's `timestamp` key
    /// written as `ts` the JSON pin failed; with `target` and `reason` swapped in
    /// `report_preimage` the preimage pin failed.
    #[test]
    fn the_evidence_text_its_hash_and_the_preimage_match_the_pins() {
        let json = evidence_json(&pinned_items());
        assert_eq!(json, PINNED_EVIDENCE_JSON);
        assert_eq!(evidence_hash(&json), PINNED_EVIDENCE_HASH);
        assert_eq!(evidence_hash(""), BLAKE3_OF_NOTHING, "the real BLAKE3, lowercase hex");
        assert_eq!(report_preimage("AABB", "CCDD", "harassment", BLAKE3_OF_NOTHING, 1_760_000_000_000), PINNED_REPORT_PREIMAGE);
    }

    /// The frame carries the evidence text byte for byte, so its hash is the signed one, and the
    /// signature checks against the reporter's own key over the 10e preimage. Seen red
    /// 2026-10-09 by hashing a spaced copy of the evidence text in `build_report_frame` (as a
    /// pretty-printer writes it) while the frame carried the compact one: the signature over
    /// the text the relay receives no longer verified.
    #[test]
    fn a_built_report_is_signed_over_the_evidence_exactly_as_sent() {
        let (seed, reporter) = identity(31);
        let draft = ReportDraft {
            target: "CCDD".into(),
            context: ReportContext::Dm,
            reason: "harassment".into(),
            note: "since Tuesday".into(),
            evidence: pinned_items(),
            ts: 1_791_504_000_000,
        };
        let frame = build_report_frame(&seed, &reporter, &draft).expect("a frame");
        let v: Value = serde_json::from_str(&frame).expect("the frame is JSON");
        assert_eq!(v["type"], "report_v2");
        assert_eq!((v["target"].as_str(), v["context"].as_str(), v["reason"].as_str()), (Some("CCDD"), Some("dm"), Some("harassment")));
        assert_eq!((v["note"].as_str(), v["ts"].as_u64()), (Some("since Tuesday"), Some(1_791_504_000_000)));
        // The evidence text as the relay receives it: from `"evidence":` to the report's own
        // `,"ts":` (the last one; the items carry `ts` keys of their own).
        let start = frame.find("\"evidence\":").unwrap() + "\"evidence\":".len();
        let end = frame.rfind(",\"ts\":").unwrap();
        let sent = &frame[start..end];
        assert_eq!(sent, PINNED_EVIDENCE_JSON, "the evidence goes out exactly as built");
        let preimage = report_preimage(&reporter, "CCDD", "harassment", &evidence_hash(sent), 1_791_504_000_000);
        let sig = B64.decode(v["sig"].as_str().unwrap()).unwrap();
        let key = hex::decode(&reporter).unwrap();
        assert!(verify_dilithium(&key, preimage.as_bytes(), &sig).is_ok(), "the reporter's signature checks over what was sent");
        let (_, someone_else) = identity(32);
        assert!(verify_dilithium(&hex::decode(someone_else).unwrap(), preimage.as_bytes(), &sig).is_err());
    }

    /// 10e's limits, refused before anything is signed. Seen red 2026-10-09 by deleting the
    /// self-report line from `check_draft`: a report naming ourselves was built.
    #[test]
    fn a_draft_outside_the_limits_is_refused() {
        let (seed, me) = identity(33);
        let base = ReportDraft { target: "CCDD".into(), context: ReportContext::Post, reason: "spam".into(), note: String::new(), evidence: vec![], ts: 1 };
        let refused = |d: ReportDraft| build_report_frame(&seed, &me, &d).err();
        assert_eq!(refused(ReportDraft { target: me.clone(), ..base.clone() }), Some(DraftProblem::SelfReport));
        assert_eq!(refused(ReportDraft { target: String::new(), ..base.clone() }), Some(DraftProblem::NoTarget));
        assert_eq!(refused(ReportDraft { reason: String::new(), ..base.clone() }), Some(DraftProblem::NoReason));
        assert_eq!(refused(ReportDraft { note: "x".repeat(501), ..base.clone() }), Some(DraftProblem::NoteTooLong));
        assert_eq!(refused(ReportDraft { note: "\u{e9}".repeat(500), ..base.clone() }), None, "500 characters, not bytes");
        let item = |n: u64| Evidence::Post { from: "CCDD".into(), timestamp: n };
        assert_eq!(refused(ReportDraft { evidence: (0..21).map(item).collect(), ..base.clone() }), Some(DraftProblem::TooManyItems));
        assert_eq!(refused(ReportDraft { evidence: (0..20).map(item).collect(), ..base.clone() }), None);
        let long = Evidence::GroupText { from: "CCDD".into(), ts: 1, text: "y".repeat(MAX_EVIDENCE_BYTES) };
        assert_eq!(refused(ReportDraft { evidence: vec![long], ..base.clone() }), Some(DraftProblem::EvidenceTooLarge));
        assert_eq!(build_report_frame(&[], &me, &base).err(), Some(DraftProblem::Locked));
    }

    /// A message kept before step D has no signature, proves nothing, and is not offered; the
    /// tick goes to the newest message that is. Seen red 2026-10-09 with the `sig` filter taken
    /// out of `dm_candidates`: the unsigned message was offered, and ticked as the newest.
    #[test]
    fn a_message_kept_without_its_signature_is_not_offered() {
        let convo = vec![stored("them", "me", 100, "signed", "s1"), stored("them", "me", 200, "kept before step D", "")];
        let c = dm_candidates(&convo, "them", "me", None);
        assert_eq!(c.iter().map(|c| (c.text.as_str(), c.ticked)).collect::<Vec<_>>(), [("signed", true)]);
        assert!(dm_candidates(&convo, "them", "me", Some(200))[0].ticked, "pressed on the unsigned one: the newest offered is ticked");
    }

    /// A message with a file carries the key that opens the file, and the signature covers it,
    /// so it is never offered: an admin is never handed the means to open what could be abuse
    /// imagery. Any version of the marker, anywhere in the text, counts (as on web). Seen red
    /// 2026-10-09 twice: with the `carries_file` filter taken out of `dm_candidates`, the file
    /// messages were offered and the newest ticked; with `carries_file` testing only for
    /// `dm_pq::FILE_MARKER` at the start, a marker later in the text was missed.
    #[test]
    fn a_message_with_a_file_is_never_offered() {
        let att = crate::net::dm_pq::DmAttachment {
            url: "https://example.invalid/u/1.enc".into(),
            k: "a2V5".into(),
            n: "bm9uY2U=".into(),
            name: "photo.jpg".into(),
            mime: "image/jpeg".into(),
            size: 10,
        };
        let marker = crate::net::dm_pq::build_file_marker(&att);
        assert!(marker.starts_with(crate::net::dm_pq::FILE_MARKER) && carries_file(&marker));
        assert!(carries_file(&format!("look {marker}")), "anywhere in the text");
        assert!(carries_file("[[hum:file:v2]]e30="), "any version of the marker");
        assert!(!carries_file("just words") && !carries_file("[[hum:follow]]"));
        let convo = vec![
            stored("them", "me", 100, "words", "s1"),
            stored("them", "me", 200, &marker, "s2"),
            stored("them", "me", 300, "see this [[hum:file:v2]]e30=", "s3"),
        ];
        let c = dm_candidates(&convo, "them", "me", None);
        assert_eq!(c.iter().map(|c| (c.text.as_str(), c.ticked)).collect::<Vec<_>>(), [("words", true)]);
    }

    /// THE picker's choice. Seen red 2026-10-09 with the `m.to == me` condition removed from
    /// `dm_candidates`: a message the target sent to someone else (a stray in the store) was
    /// offered.
    #[test]
    fn the_picker_offers_only_their_messages_to_us_newest_first_with_the_right_one_ticked() {
        let convo = vec![
            stored("them", "me", 100, "first", "s1"),
            stored("me", "them", 150, "my reply", "s2"),
            stored("them", "me", 300, "third", "s3"),
            stored("them", "me", 200, "second", "s4"),
            stored("them", "else", 250, "to someone else", "s5"),
        ];
        let c = dm_candidates(&convo, "them", "me", None);
        assert_eq!(c.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(), ["third", "second", "first"]);
        assert_eq!(c.iter().map(|c| c.ticked).collect::<Vec<_>>(), [true, false, false], "the most recent is ticked");
        assert_eq!(c[0].item, Evidence::Dm { from: "them".into(), to: "me".into(), ts: 300, text: "third".into(), sig: "s3".into() });
        // Opened from the menu of one of their messages: that one is ticked instead.
        let pressed = dm_candidates(&convo, "them", "me", Some(100));
        assert_eq!(pressed.iter().map(|c| c.ticked).collect::<Vec<_>>(), [false, false, true]);
        // Opened from our own message's time (not theirs): the most recent again.
        assert!(dm_candidates(&convo, "them", "me", Some(150))[0].ticked);
        // Ticked items go out oldest first.
        let mut all = c.clone();
        all.iter_mut().for_each(|c| c.ticked = true);
        assert_eq!(chosen(&all).iter().map(|e| match e { Evidence::Dm { ts, .. } => *ts, _ => 0 }).collect::<Vec<_>>(), [100, 200, 300]);
        assert!(dm_candidates(&convo, "me", "me", None).is_empty(), "never ourselves");
        assert!(dm_candidates(&[], "them", "me", None).is_empty());
    }

    /// The reasons file, both shapes; an empty or broken one is an error. Seen red 2026-10-09 by
    /// making `parse_reasons`'s `Wrapped` arm return an empty list: the object form failed to load.
    #[test]
    fn the_reasons_load_from_a_fixture_in_either_shape() {
        let list = br#"[{"id":"spam","label":"Spam","help":"Unwanted ads."},{"id":"child_danger","label":"A child may be in danger","help":"If anyone is in danger right now, contact your local emergency number. This server's admins are volunteers, not police."}]"#;
        let r = parse_reasons(list).unwrap();
        assert_eq!(r.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["spam", "child_danger"]);
        assert!(r[1].help.contains("volunteers, not police"));
        let wrapped = br#"{"reasons":[{"id":"other","label":"Something else"}]}"#;
        assert_eq!(parse_reasons(wrapped).unwrap()[0].help, "");
        assert!(parse_reasons(b"[]").is_err());
        assert!(parse_reasons(b"not json").is_err());
        // The loader reads `safety/report_reasons.json` under the data folder it is given.
        let dir = crate::test_temp::dir("report-reasons");
        std::fs::create_dir_all(dir.path().join("safety")).unwrap();
        std::fs::write(dir.path().join(REASONS_FILE), list).unwrap();
        assert_eq!(load_reasons(dir.path()).unwrap().len(), 2);
    }

    /// The admins' list in the relay's shapes: a moderator's open DM report (reporter "a member",
    /// DM items without `to` and `sig`) and an admin's decided post report; the badges come from
    /// `checked` alone. Seen red 2026-10-09 by loosening `badge`'s first arm from `(true, "dm")`
    /// to `(_, "dm")`: the forged, unchecked DM item was badged "Signature checked".
    #[test]
    fn the_listed_reports_read_and_their_badges_say_what_was_proven() {
        let frame = serde_json::json!({ "type": "reports", "state": "open", "items": [
            { "id": 7, "target": "them", "target_name": "Dana", "context": "dm", "reason": "harassment",
              "reason_label": "Harassment", "note": "n", "created_at": 1_791_504_000_000u64, "state": "open",
              "decision": null, "decision_note": null, "decided_by": null, "decided_by_name": null, "decided_at": null,
              "reporter_name": "a member",
              "evidence": [ { "kind": "dm", "from": "them", "ts": 5, "text": "go away", "checked": true },
                            { "kind": "dm", "from": "them", "ts": 6, "text": "forged", "checked": false } ] },
            { "id": 8, "target": "x", "target_name": "Xan", "context": "post", "reason": "spam", "reason_label": "Spam",
              "note": "", "created_at": 1_791_504_100_000u64, "state": "decided",
              "decision": "delete_post", "decision_note": "gone", "decided_by": "modkey", "decided_by_name": "Mo",
              "decided_at": 1_791_504_200_000u64, "reporter": "", "reporter_name": "an erased account",
              "evidence": [ { "kind": "post", "from": "x", "timestamp": 4, "text": "buy", "checked": true } ] },
            { "context": "dm" }
        ]});
        let list = parse_reports(&frame);
        assert_eq!(list.len(), 2, "an item with no id is skipped");
        let r = &list[0];
        assert_eq!((r.id_text(), r.reason_label.as_str(), r.reporter.as_str(), r.reporter_name.as_str()), ("7".to_string(), "Harassment", "", "a member"));
        assert!(!r.is_decided() && r.decision.is_empty() && r.decided_at == 0, "nulls read as not decided");
        assert_eq!(as_millis(r.created_at), 1_791_504_000_000);
        assert_eq!(r.evidence[0].to, "", "a moderator is not sent who it was to");
        assert_eq!(badge(&r.evidence[0], "Dana"), "Signature checked: sent by Dana to the reporter");
        assert_eq!(badge(&r.evidence[1], "Dana"), "Not proven");
        let r = &list[1];
        assert_eq!((r.decision.as_str(), r.decision_note.as_str(), r.decided_by_name.as_str()), ("delete_post", "gone", "Mo"));
        assert_eq!(r.reporter_name, "an erased account");
        assert!(r.is_decided() && r.has_post());
        assert_eq!(r.evidence[0].ts, 4, "a post's time is its timestamp");
        assert_eq!(badge(&r.evidence[0], "Xan"), "Found on this server: posted by Xan");
        assert_eq!(as_millis(1_791_504_000), 1_791_504_000_000, "a time in seconds is read as seconds");
        assert_eq!(Decision::label_of("delete_post"), "Delete post");
        let decide: Value = serde_json::from_str(&decide_frame(&list[0].id, Decision::Warn, "first time")).unwrap();
        assert_eq!((decide["type"].as_str(), decide["id"].as_u64(), decide["decision"].as_str()), (Some("report_decide"), Some(7), Some("warn")));
        assert_eq!(serde_json::from_str::<Value>(&list_frame(true)).unwrap()["state"], "decided");
    }
}
