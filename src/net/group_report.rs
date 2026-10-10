//! A report about a group reaches the group's creator (section 10j of
//! docs/design/blocking-and-safe-mode.md, 2026-10-10): the desktop app's rules, with no socket and
//! no GUI, so each one is unit tested. The web client built this first and its words are the ones
//! used here, word for word (web/shared/group-report.js); where the two must agree byte for byte
//! (the marker and its JSON), scripts/tests/fixtures/group-report-marker.json holds both to it.
//!
//! WHY A GROUP'S CREATOR: a peer-to-peer group's messages are encrypted for the group, so a
//! server's admins can neither read them nor check who wrote them, and they cannot remove anyone
//! from the group. The creator holds the group's messages in their own copy and is the one who
//! admits and removes members, so a member can send a report to them instead (or as well).
//!
//! WHAT GOES TO THE CREATOR: an ordinary signed, sealed v2 DM (the server sees nothing of it),
//! deposited with `"group_report": true` on the `dm_put` so the creator's server lets it past
//! their "who can reach me" setting when the two share a group the creator made (3 a day, never
//! under Nobody). Its text is `[[hum:group-report:v1]]` followed by the JSON
//! `{"group_id","group_name","target","reason","note","items":[{"id","from","ts","text"}]}`, in
//! that order, each item naming a group message by its signed-object id and repeating its sender,
//! time and text as the reporter's copy shows them. At most 20 items and 16 KB of text, and never
//! a message with a file (step D's rule, `report::carries_file`). The DM's own signature says who
//! sent it, so the reporter is named to the creator, and the dialog says so before sending.
//!
//! WHAT THE CREATOR'S APP DOES WITH ONE (`resolve`): keeps it only when it is addressed to them,
//! about a group they hold AS ITS CREATOR (a group they only joined counts as not held), from a
//! current member, and not about them. Each item is checked against their OWN copy of the group
//! (`check_items`): found needs the stored object's own bytes to hash to the item's id, its
//! signature to check, the group as its first reference, and the same sender, time and text.
//! Anything else is "Not found in your copy", shown and never dropped. Kept reports live in the
//! local encrypted DM store (net/dm_store.rs) until dismissed and are never sent to any server.
//!
//! The app-state glue (the dialog's creator lookup, sending, the refusal, the checks on arrival
//! and the three actions) is src/engine/group_report.rs; removing someone, which changes the
//! group key first, is src/net/group_remove.rs; the Safety section is
//! src/gui/pages/safety_group_reports.rs.

use std::collections::HashMap;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::api_v2::P2pGroupInfo;
use super::dm_pq::DmInner;
use super::report::{carries_file, FILES_NOT_INCLUDED, MAX_NOTE_CHARS};

/// The marker a report's text starts with. Must match the web client.
pub const MARKER: &str = "[[hum:group-report:v1]]";
/// At most this many group messages in one report (10j).
pub const MAX_ITEMS: usize = 20;
/// At most this many bytes of text (the marker and the JSON, in UTF-8) in one report (10j).
pub const MAX_BYTES: usize = 16 * 1024;
/// A `reach_refused` for the creator this soon after a report to them is about the report (the
/// web client's minute): nothing else in the refusal tells it from an ordinary message.
pub const REFUSAL_WINDOW: Duration = Duration::from_secs(60);
/// The largest whole number a JavaScript number holds exactly: the web client reads an item's
/// time as one, so a time past it is not one the web would accept either.
const MAX_SAFE_INTEGER: u64 = (1 << 53) - 1;

// ── Words (web/shared/group-report.js, word for word) ────────────────────────────────────────

pub const SEND_TO: &str = "Send this report to";
/// The two cases 10j names where only the admins remain, and the web's third: the creator could
/// not be found out here (the group's own record did not load, or its signature did not check).
pub const YOU_CREATED: &str = "You created this group: remove them from the group's member list.";
pub const THEY_CREATED: &str = "The person you are reporting created this group, so this goes to the server's admins.";
pub const CREATOR_UNKNOWN: &str = "Who created this group could not be found out here, so this goes to the server's admins.";
pub const FINDING_CREATOR: &str = "Finding out who created this group...";
/// Said before sending whenever the creator is chosen (10j).
pub const CREATOR_SEES: &str = "The group's creator will see that you sent this.";
/// Under the group message, when the creator is chosen (beside step D's not-proven line, which
/// stays for the admins).
pub const CREATOR_CHECKS: &str = "The group's creator can check these words against their own copy of the group.";
pub const SENT_LINE: &str = "Report sent to the group's creator.";
pub const REFUSED_LINE: &str =
    "Your report did not reach the group's creator: this server did not let it through. You can send it to this server's admins instead.";
pub const STILL_FINDING: &str = "Still finding out who created this group. Try again in a moment.";
pub const CANNOT_SEAL: &str = "The group's creator has not been online with a current client here, so the report cannot be sealed for them yet. Try again later, or send it to this server's admins.";

// The creator's side: Settings > Safety.
pub const TITLE: &str = "Reports about your groups";
pub const HELP: &str = "Members of a group you created can report one of its messages to you. Each report names who sent it. Reports stay on this device until you dismiss them, and are never sent to any server.";
pub const NONE: &str = "No reports about your groups.";
pub const NOT_FOUND: &str = "Not found in your copy";
pub const ACTION_REMOVE: &str = "Remove them from the group";
pub const ACTION_BLOCK: &str = "Block them";
pub const ACTION_DISMISS: &str = "Dismiss";
pub const REMOVED: &str = "Removed from the group.";
pub const NOT_IN_GROUP: &str = "They are not in the group now.";
pub const BLOCKED: &str = "You have blocked them.";
pub const NO_ITEMS: &str = "No messages were included.";

/// The badge on an item found in the creator's copy (10j).
pub fn found_badge(name: &str) -> String {
    format!("Found in your copy of the group, signed by {name}")
}

/// The line the creator is shown when a report arrives.
pub fn arrived_line(reporter: &str, target: &str, group: &str) -> String {
    format!("{reporter} sent you a report about {target} in {group}. It is in Safety, under Reports about your groups.")
}

/// The count on the group in the group list: what hovering it says.
pub fn count_title(n: usize) -> String {
    if n == 1 {
        "1 report about this group. See Reports about your groups in Safety.".to_string()
    } else {
        format!("{n} reports about this group. See Reports about your groups in Safety.")
    }
}

/// The confirmation before a removal (it cannot be undone: they need a new invite ticket).
pub fn remove_confirm(name: &str, group: &str) -> String {
    format!("Remove {name} from \"{group}\"? They can come back only with a new invite ticket.")
}

/// After a removal went through.
pub fn removed_line(name: &str, group: &str) -> String {
    format!("{name} was removed from {group}.")
}

/// The dialog's first line when the report goes to the creator (10j's "As built"): None for the
/// admins alone, whose line the dialog keeps as step D wrote it.
pub fn lead_line(dest: Option<Dest>, name: &str) -> Option<String> {
    match dest {
        Some(Dest::Creator) => Some(format!("This goes to the group's creator. {name} is not told who reported them.")),
        Some(Dest::Both) => Some(format!(
            "This goes to the group's creator and to this server's admins and moderators. {name} is not told who reported them."
        )),
        _ => None,
    }
}

// ── Who the report goes to ───────────────────────────────────────────────────────────────────

/// Where a report about a group message goes (10j), in the dialog's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dest {
    Creator,
    Admins,
    Both,
}

impl Dest {
    pub const ALL: [Dest; 3] = [Dest::Creator, Dest::Admins, Dest::Both];

    pub fn label(self) -> &'static str {
        match self {
            Dest::Creator => "The group's creator",
            Dest::Admins => "This server's admins",
            Dest::Both => "Both",
        }
    }

    pub fn to_creator(self) -> bool {
        matches!(self, Dest::Creator | Dest::Both)
    }

    pub fn to_admins(self) -> bool {
        matches!(self, Dest::Admins | Dest::Both)
    }
}

/// What the dialog offers for a group message: the destinations in the dialog's order, the
/// default, and the sentence shown when only the admins remain. While the creator is being found
/// nothing is offered and Send waits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choices {
    pub choices: Vec<Dest>,
    pub chosen: Option<Dest>,
    pub line: Option<&'static str>,
    pub finding: bool,
}

/// THE dialog's choice (10j and the web's third case). `creator` is the group's creator once
/// found (None when it could not be found out).
pub fn choices(me: &str, target: &str, creator: Option<&str>, finding: bool) -> Choices {
    if finding {
        return Choices { choices: Vec::new(), chosen: None, line: Some(FINDING_CREATOR), finding: true };
    }
    let admins_only = |line| Choices { choices: vec![Dest::Admins], chosen: Some(Dest::Admins), line: Some(line), finding: false };
    let Some(creator) = creator.and_then(key_norm) else { return admins_only(CREATOR_UNKNOWN) };
    if same_key(&creator, me) {
        return admins_only(YOU_CREATED);
    }
    if same_key(&creator, target) {
        return admins_only(THEY_CREATED);
    }
    Choices { choices: Dest::ALL.to_vec(), chosen: Some(Dest::Creator), line: None, finding: false }
}

/// The open Report dialog's part for a P2P group message (`ReportDialog::group`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GroupTarget {
    /// The group's id, and its name as this app's group list has it.
    pub id: String,
    pub name: String,
    /// The message the creator checks; None for a message with a file (step D's rule) or one
    /// whose signed-object id is not known here.
    pub item: Option<Item>,
    /// The group's creator, once found from the group's own signed record.
    pub creator: Option<String>,
    /// True while the creator is being found.
    pub finding: bool,
    /// What the person picked under "Send this report to" (None: the default).
    pub send_to: Option<Dest>,
}

impl GroupTarget {
    pub fn choices(&self, me: &str, target: &str) -> Choices {
        choices(me, target, self.creator.as_deref(), self.finding)
    }

    /// Where the report goes now: the pick when it is still offered, else the default. None
    /// while the creator is being found.
    pub fn destination(&self, me: &str, target: &str) -> Option<Dest> {
        let c = self.choices(me, target);
        if c.finding {
            return None;
        }
        match self.send_to {
            Some(d) if c.choices.contains(&d) => Some(d),
            _ => c.chosen,
        }
    }

    /// The creator lookup finished (None: it could not be found out).
    pub fn found(&mut self, creator: Option<String>) {
        self.creator = creator.as_deref().and_then(key_norm);
        self.finding = false;
        self.send_to = None;
    }
}

// ── Keys, ids, the marker ────────────────────────────────────────────────────────────────────

/// JavaScript's `String.prototype.trim` set: the web trims with it, so a note trimmed here is
/// the note the web would have sent (Rust's `trim` differs at U+0085 and U+FEFF).
fn js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'..='\u{200A}' | '\u{2028}'
            | '\u{2029}' | '\u{202F}' | '\u{205F}' | '\u{3000}' | '\u{FEFF}'
    )
}

fn js_trim(s: &str) -> &str {
    s.trim_matches(js_space)
}

/// A person's key in its one spelling: lowercase hex, 16 to 8192 digits (step D's rule on both
/// clients). None for anything else.
pub fn key_norm(key: &str) -> Option<String> {
    let k = js_trim(key).to_ascii_lowercase();
    ((16..=8192).contains(&k.len()) && k.bytes().all(|b| b.is_ascii_hexdigit())).then_some(k)
}

/// A group's id, or a group message's: the BLAKE3 of the signed object, 64 lowercase hex digits.
pub fn id_norm(id: &str) -> Option<String> {
    let v = js_trim(id).to_ascii_lowercase();
    (v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit())).then_some(v)
}

fn same_key(a: &str, b: &str) -> bool {
    matches!((key_norm(a), key_norm(b)), (Some(x), Some(y)) if x == y)
}

/// A reason id from data/safety/report_reasons.json: lowercase letters, digits and underscores.
fn reason_ok(r: &str) -> bool {
    (1..=64).contains(&r.len()) && r.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// Step D's note rule, as the web applies it: trimmed, at most 500 characters.
fn note_norm(note: &str) -> String {
    js_trim(note).chars().take(MAX_NOTE_CHARS).collect()
}

/// A JSON string exactly as JavaScript's `JSON.stringify` writes it (serde_json escapes the same
/// characters the same way: the quote, the backslash and the control characters, lowercase hex).
fn json_str(s: &str) -> String {
    Value::from(s).to_string()
}

/// One group message as an item: its signed-object id, its sender, time and text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub from: String,
    pub ts: u64,
    pub text: String,
}

/// A group message as an item, or None when its id or sender is not one, its time is past what
/// the web can read, or its text carries a file (step D's rule: the text would hand over the key
/// that opens the file).
pub fn item(id: &str, from: &str, ts: u64, text: &str) -> Option<Item> {
    let id = id_norm(id)?;
    let from = key_norm(from)?;
    if ts > MAX_SAFE_INTEGER || carries_file(text) {
        return None;
    }
    Some(Item { id, from, ts, text: text.to_string() })
}

/// A report's fields: what the dialog hands the builder, and what a received one reads as.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub group_id: String,
    pub group_name: String,
    pub target: String,
    pub reason: String,
    pub note: String,
    pub items: Vec<Item>,
}

/// The text of a report to the creator: the marker, then the JSON in 10j's order, byte for byte
/// what the web builds for the same fields (the shared fixture holds both to it). Err is the
/// sentence the dialog shows, the web's words.
pub fn build_text(f: &Report) -> Result<String, &'static str> {
    let group_id = id_norm(&f.group_id).ok_or("This group is not known here.")?;
    let target = key_norm(&f.target).ok_or("This person cannot be reported from here: their key is not known.")?;
    if !reason_ok(&f.reason) {
        return Err("Choose a reason first.");
    }
    if f.items.len() > MAX_ITEMS {
        return Err("At most 20 messages can be included. Untick some.");
    }
    let mut items = Vec::with_capacity(f.items.len());
    for it in &f.items {
        if carries_file(&it.text) {
            return Err(FILES_NOT_INCLUDED);
        }
        let it = item(&it.id, &it.from, it.ts, &it.text).ok_or("A group message is not named.")?;
        items.push(format!(
            "{{\"id\":{},\"from\":{},\"ts\":{},\"text\":{}}}",
            json_str(&it.id),
            json_str(&it.from),
            it.ts,
            json_str(&it.text)
        ));
    }
    let note = note_norm(&f.note);
    if carries_file(&note) {
        return Err(FILES_NOT_INCLUDED);
    }
    let text = format!(
        "{MARKER}{{\"group_id\":{},\"group_name\":{},\"target\":{},\"reason\":{},\"note\":{},\"items\":[{}]}}",
        json_str(&group_id),
        json_str(&f.group_name),
        json_str(&target),
        json_str(&f.reason),
        json_str(&note),
        items.join(",")
    );
    if text.len() > MAX_BYTES {
        return Err("This report is too long to send to the group's creator (16 KB at most). Shorten the note or include fewer messages.");
    }
    Ok(text)
}

/// Is this DM text a report to a group's creator (whatever follows the marker)? Such a text is
/// never shown or stored as a message.
pub fn is_report_text(text: &str) -> bool {
    text.starts_with(MARKER)
}

/// A JSON number read as the web reads `Number(ts)`: a whole number from 0 to 2^53 - 1.
fn whole(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| v.as_f64().filter(|f| f.fract() == 0.0 && *f >= 0.0).map(|f| f as u64)).filter(|n| *n <= MAX_SAFE_INTEGER)
}

/// Read a received report's text: its fields, normalised, or None when it is not one this app
/// (or the web) would send: the wrong shape, more than 20 items, over 16 KB, or a file anywhere.
pub fn parse(text: &str) -> Option<Report> {
    let rest = text.strip_prefix(MARKER)?;
    if text.len() > MAX_BYTES || carries_file(text) {
        return None;
    }
    let v: Value = serde_json::from_str(rest).ok()?;
    let o = v.as_object()?;
    let group_id = id_norm(o.get("group_id")?.as_str()?)?;
    let target = key_norm(o.get("target")?.as_str()?)?;
    let reason = o.get("reason")?.as_str()?.to_string();
    if !reason_ok(&reason) {
        return None;
    }
    let group_name = o.get("group_name")?.as_str()?.to_string();
    let note = o.get("note")?.as_str()?.to_string();
    if note.chars().count() > MAX_NOTE_CHARS {
        return None;
    }
    let list = o.get("items")?.as_array()?;
    if list.len() > MAX_ITEMS {
        return None;
    }
    let mut items = Vec::with_capacity(list.len());
    for it in list {
        let it = it.as_object()?;
        let text = it.get("text")?.as_str()?;
        items.push(item(it.get("id")?.as_str()?, it.get("from")?.as_str()?, whole(it.get("ts")?)?, text)?);
    }
    Some(Report { group_id, group_name, target, reason, note, items })
}

// ── The creator's side ───────────────────────────────────────────────────────────────────────

/// Why a received report is not kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    NotToMe,
    NotHeld,
    NotCreator,
    ReporterNotInGroup,
    AboutMe,
}

/// Does the creator keep this report? `from` and `to` are the DM's signed sender and recipient,
/// `groups` this app's groups as the server lists them now. Returns the group it is about.
pub fn accepts<'a>(me: &str, from: &str, to: &str, report: &Report, groups: &'a [P2pGroupInfo]) -> Result<&'a P2pGroupInfo, Why> {
    if !same_key(to, me) {
        return Err(Why::NotToMe);
    }
    let group = groups.iter().find(|g| id_norm(&g.group_id).as_deref() == Some(report.group_id.as_str())).ok_or(Why::NotHeld)?;
    if !group.is_creator {
        return Err(Why::NotCreator);
    }
    if !group.members.iter().any(|m| same_key(m, from)) {
        return Err(Why::ReporterNotInGroup);
    }
    if same_key(&report.target, me) {
        return Err(Why::AboutMe);
    }
    Ok(group)
}

/// One item after the check against the creator's own copy: found, and who signed it when found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckedItem {
    pub id: String,
    pub from: String,
    pub ts: u64,
    pub text: String,
    pub found: bool,
    #[serde(default)]
    pub signer: String,
}

/// Check each item against the creator's own copy of the group (10j). `objects` are the group's
/// message objects as the server serves them (`/api/v2/groups/{id}/messages`: object_id,
/// object_type, author_public_key_b64, created_at, references, payload_b64, signature_b64, ...),
/// `keys` the group keys this app holds, by epoch. An item is found only when an object named by
/// its id is there whose own bytes hash to that id (so the server cannot move an id onto other
/// bytes, even an identical message of the same person), whose signature checks, which is a group
/// message of THIS group, and whose sender, time and words (opened with the key of its epoch) are
/// the item's.
pub fn check_items(report: &Report, objects: &[Value], keys: &HashMap<u64, Vec<u8>>) -> Vec<CheckedItem> {
    report
        .items
        .iter()
        .map(|it| {
            let signer = objects
                .iter()
                .filter(|o| o.get("object_id").and_then(|x| x.as_str()).is_some_and(|id| id.eq_ignore_ascii_case(&it.id)))
                .find_map(|o| {
                    // The signature is checked over the object's own bytes, and the id recomputed.
                    let v = super::api_v2::verify_submission_json(&o.to_string())?;
                    let ok = v.object_id == it.id
                        && v.object_type == "group_msg_v1"
                        && v.references.first() == Some(&report.group_id)
                        && same_key(&v.author_pubkey_hex, &it.from)
                        && u64::try_from(v.created_at).ok() == Some(it.ts);
                    let epoch = super::group_e2ee::parse_group_msg_epoch(&v.payload).ok()?;
                    let words = super::group_e2ee::open_group_msg(&v.payload, keys.get(&epoch)?).ok()?;
                    (ok && words == it.text).then(|| v.author_pubkey_hex.to_ascii_lowercase())
                });
            CheckedItem { id: it.id.clone(), from: it.from.clone(), ts: it.ts, text: it.text.clone(), found: signer.is_some(), signer: signer.unwrap_or_default() }
        })
        .collect()
}

/// The badge on one checked item, and whether it is the found kind.
pub fn badge(item: &CheckedItem, name: &str) -> (bool, String) {
    if item.found {
        (true, found_badge(name))
    } else {
        (false, NOT_FOUND.to_string())
    }
}

/// A report waiting for its check (it needs the server's group list and the group's messages,
/// which are fetched off the frame). Kept in the DM store so closing the app cannot lose one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingReport {
    pub id: String,
    pub from: String,
    pub to: String,
    pub ts: u64,
    pub report: Report,
}

/// A report kept on this device: who sent it and when, what it is about, and each item as the
/// check found it. `removed` once "Remove them from the group" went through.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeptReport {
    pub id: String,
    pub from: String,
    pub ts: u64,
    pub group_id: String,
    pub group_name: String,
    pub target: String,
    pub reason: String,
    pub note: String,
    pub items: Vec<CheckedItem>,
    #[serde(default)]
    pub removed: bool,
}

/// One record per report, however often the mailbox hands it over: the BLAKE3 of the DM's
/// signature (unique per sender, recipient, time and text).
pub fn record_id(sig_b64: &str) -> String {
    blake3::hash(format!("group-report\n{sig_b64}").as_bytes()).to_hex().to_string()
}

/// A verified DM carrying a report, as a report to check; None for our own (from this or another
/// of our devices: nothing to keep), one that does not read, or one without its signature.
pub fn pending_from(me: &str, inner: &DmInner) -> Option<PendingReport> {
    if !is_report_text(&inner.text) || same_key(&inner.from, me) || inner.sig_b64.is_empty() {
        return None;
    }
    let report = parse(&inner.text)?;
    Some(PendingReport {
        id: record_id(&inner.sig_b64),
        from: inner.from.to_ascii_lowercase(),
        to: inner.to.to_ascii_lowercase(),
        ts: inner.ts,
        report,
    })
}

/// THE creator's decision on a report: kept (each item checked against the copy given) or why
/// not. The group's name is the one the creator's own list has, never the report's claim.
pub fn resolve(me: &str, p: &PendingReport, groups: &[P2pGroupInfo], objects: &[Value], keys: &HashMap<u64, Vec<u8>>) -> Result<KeptReport, Why> {
    let group = accepts(me, &p.from, &p.to, &p.report, groups)?;
    let r = &p.report;
    Ok(KeptReport {
        id: p.id.clone(),
        from: p.from.clone(),
        ts: p.ts,
        group_id: r.group_id.clone(),
        group_name: if group.name.is_empty() { r.group_name.clone() } else { group.name.clone() },
        target: r.target.clone(),
        reason: r.reason.clone(),
        note: r.note.clone(),
        items: check_items(r, objects, keys),
        removed: false,
    })
}

/// How many kept reports are about this group (the count on it in the group list).
pub fn count(reports: &[KeptReport], group_id: &str) -> usize {
    reports.iter().filter(|r| r.group_id.eq_ignore_ascii_case(group_id)).count()
}

/// The group's creator, from the group's own signed `group_v1` record as the server serves it
/// (`/api/v2/objects/{id}`): its signature checked and its bytes hashing to the group's id, so a
/// server cannot name someone else. None when the record is not that.
pub fn creator_from_group_object(obj: &Value, group_id: &str) -> Option<String> {
    let v = super::api_v2::verify_submission_json(&obj.to_string())?;
    (v.object_type == "group_v1" && v.object_id.eq_ignore_ascii_case(group_id)).then(|| v.author_pubkey_hex.to_ascii_lowercase())
}

// ── State the app keeps (`ReportUi::group`) ──────────────────────────────────────────────────

/// What the check of one pending report came to.
#[derive(Debug)]
pub enum CheckOutcome {
    Kept(KeptReport),
    Dropped(Why),
    /// Neither the server's group list nor an earlier one could be read: try again later.
    Retry,
}

/// The app's part for 10j: lookups, checks and removals that run off the frame, and what the
/// Safety section is in the middle of.
#[derive(Debug, Default)]
pub struct GroupReportUi {
    /// Group id -> its creator's key, once found. A group's creator never changes.
    pub creators: HashMap<String, String>,
    /// The open dialog's creator lookup: the group's id, and where its answer arrives.
    pub finding: Option<(String, Receiver<Option<String>>)>,
    /// Creator key -> when a report to them was sent, for `REFUSAL_WINDOW`.
    pub sent: HashMap<String, Instant>,
    /// Checks under way: the server they run against, the report's id, the answer.
    pub checking: Vec<(String, String, Receiver<CheckOutcome>)>,
    /// A check that could not run: not again before this.
    pub retry_at: HashMap<String, Instant>,
    /// Removals under way: the report's id and the answer (the new key and its epoch, or why not).
    pub removing: Vec<(String, Receiver<Result<(u64, Vec<u8>), super::group_remove::RemoveError>>)>,
    /// The report whose "Remove them from the group" is asking to be confirmed.
    pub confirm_remove: Option<String>,
}

#[cfg(test)]
#[path = "group_report_tests.rs"]
mod tests;
