//! "Report a copyright problem" on the Files page, and the public takedown log (2026-10-13).
//!
//! WHY. The operator has no address for a registered copyright agent yet and prefers in-app
//! notices to an email inbox (his answers of 2026-10-13): "a simple form so it's easy... a
//! dedicated button somewhere on the media page... We can log the reports publicly." So anyone,
//! signed in or not, can tell this server's admins that an item on its Files page is their work,
//! shared without their permission. The admins are told inside HumanityOS; an admin removes the
//! item or keeps it, with a note; and every decision goes into a public log that names the item,
//! the dates, the outcome and the note, NEVER the person who reported or how to reach them (the
//! practice of the Lumen database, docs/reference/findings/2026-10-12-project-entity-and-
//! succession.md, section 3). Members also keep the in-app Report with the reason `copyright`
//! (handlers/reports.rs); this form is for everyone else, and for members who prefer it.
//!
//! THE ROUTES:
//! - `POST /api/files/takedown`, no sign-in: `{item, name, contact, statement, good_faith}`.
//!   `item` is the item's number; `name` (at most [`NAME_MAX`] characters) and `contact` (at most
//!   [`CONTACT_MAX`]) are optional; `statement` (why the work is theirs, at most
//!   [`STATEMENT_MAX`]) is required; `good_faith` must be `true`, the ticked box ([`GOOD_FAITH_LINE`]).
//!   Answers `{ok: true, id, words}`, or a refusal in plain words: 400 (the form is not right),
//!   404 (no such item), 429 (a limit, below).
//! - `GET /api/files/takedowns`, anyone: the public log, `{takedowns: [{id, item_id, item_title,
//!   created_at, decided_at, outcome, note}]}`, the latest decision first, at most
//!   [`PUBLIC_LOG_MAX`]. `outcome` is "removed" or "kept"; `item_title` is "" when the item was not
//!   public when it was reported (a members-only item's title never reaches the log) or had no
//!   title. Built field by field ([`public_json`]), so nothing personal can ride along.
//! - `GET` (query) or `POST` (body) `/api/files/takedowns/open`, signed by an admin or the owner
//!   with purpose `media_takedowns` (Dilithium over `purpose\ntimestamp`, the Files page's
//!   [`check_signed`]; the POST form because a signature in a URL is about 10 KB, which a proxy
//!   refuses): `{requests: [{id, item_id, item_title, item_public, item_on_page, created_at,
//!   name, contact, statement, state}]}`, oldest first.
//! - `POST /api/files/takedown/decide`, signed by an admin or the owner with purpose
//!   `media_takedown:<id>:<removed|kept>`: `{key, timestamp, sig, id, state, note}`. `removed`
//!   removes the item through the Files page's own remove path ([`remove_core`]: its picks go, a
//!   file uploaded for Media is deleted, a file shared in chat or Shared Files goes back there); an
//!   item already gone counts as removed. The note (at most [`NOTE_MAX`]) is shown in the public
//!   log. Answers `{ok, id, outcome, item_id, item_removed, item_already_gone, removal}`.
//!
//! LIMITS (the form is open to anyone, so the relay holds the line): at most
//! [`TAKEDOWNS_PER_ADDRESS_PER_DAY`] reports a UTC day from one address and
//! [`TAKEDOWNS_PER_DAY`] from everyone. The address is the `X-Real-IP` header nginx sets from the
//! connection, NEVER `X-Forwarded-For`, which a client can write; an IPv6 address counts as its
//! /64 (media_counts.rs `address_of`, the same reading the view counts use). What is held is a
//! count per MARK, the first 16 bytes of BLAKE3 keyed with a secret drawn at random each UTC day
//! and held only in memory: no address is kept, and at midnight UTC the marks, the counts and the
//! secret are all forgotten. A restart forgets them too (a few more reports that day, never a
//! record of who). A form refused for what it holds, or for naming no item on this server, costs
//! nothing; a report is counted just before it is kept.
//!
//! THE ADMINS ARE TOLD as reports reach moderators (handlers/reports.rs): a system line to every
//! admin and owner signed in now. The report waits in the Admin tab for anyone who was away.
//!
//! WHAT IS PERSONAL: the name, contact and statement. Only admins see them; nothing logs them;
//! the expiry pass blanks them 90 days after the decision (storage/takedowns.rs).

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

use crate::relay::files::{check_signed, db_err, forbidden, is_admin, media_settings, remove_core, Folders, RemoveTarget, SignedOnly};
use crate::relay::files_counts::{address_of, shown_to_everyone};
use crate::relay::relay::{RelayMessage, RelayState};
use crate::relay::storage::takedowns::{NewTakedown, TakedownRow};
use crate::relay::storage::Storage;

type Refused = (StatusCode, String);
type Answer = Result<Json<Value>, Refused>;

/// Copyright reports kept from one address in one UTC day.
pub const TAKEDOWNS_PER_ADDRESS_PER_DAY: u32 = 3;
/// Copyright reports kept from everyone in one UTC day.
pub const TAKEDOWNS_PER_DAY: u32 = 100;
/// The longest name, in characters.
pub const NAME_MAX: usize = 200;
/// The longest contact, in characters.
pub const CONTACT_MAX: usize = 300;
/// The longest statement, in characters.
pub const STATEMENT_MAX: usize = 2000;
/// The longest note an admin writes with a decision (it is public), in characters.
pub const NOTE_MAX: usize = 1000;
/// The most decisions the public log answers with.
pub const PUBLIC_LOG_MAX: usize = 500;
/// The biggest form body the route reads: a full statement in any script, JSON-escaped, fits.
pub const TAKEDOWN_BODY_LIMIT: usize = 32 * 1024;
/// The purpose an admin signs to read the open reports.
pub const OPEN_PURPOSE: &str = "media_takedowns";

/// The words beside the box the form must have ticked (both apps show them).
pub const GOOD_FAITH_LINE: &str =
    "I believe in good faith that this use is not allowed by me or the law, and what I wrote is accurate.";
/// The answer to a form whose box was not ticked.
pub const NEEDS_GOOD_FAITH: &str = "Tick the box to say you believe in good faith that this use is not allowed by you or the law, and that what you wrote is accurate. The report was not sent.";
/// The answer to a form with no statement.
pub const NEEDS_STATEMENT: &str = "Say why the work is yours: that part of the form is needed. The report was not sent.";
/// The answer to a form that names no item.
pub const NAME_THE_ITEM: &str = "Give the number of the item on the Files page (the number at the end of its address, /files/12). The report was not sent.";
/// The answer to a form that is not a form at all.
pub const NOT_A_FORM: &str = "That is not a copyright report form. The report was not sent.";
/// The answer to a form naming an item this server does not have.
pub const NO_SUCH_ITEM: &str = "There is no item with that number on this server's Files page. The report was not sent.";
/// The answer when this server has taken in its day's reports from everyone.
pub const SERVER_FULL: &str = "This server has taken in as many copyright reports today as it takes in one day, so this one was not sent. Try again after midnight UTC.";
/// The answer to a report that was kept.
pub const RECEIVED: &str = "Received. An admin will look at it. The outcome will appear in the public takedown log, without your name or contact.";

/// The answer when one address has sent its day's reports.
pub fn address_full_words() -> String {
    format!(
        "{TAKEDOWNS_PER_ADDRESS_PER_DAY} copyright reports have come from your connection today, the most one connection may send in a day, so this one was not sent. Try again after midnight UTC."
    )
}

fn bad(words: impl Into<String>) -> Refused {
    (StatusCode::BAD_REQUEST, words.into())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as i64
}

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

// ── The day's limits (in memory only) ──

/// Which limit a report ran into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Over {
    /// This address sent its [`TAKEDOWNS_PER_ADDRESS_PER_DAY`] today.
    Address,
    /// The server took in its [`TAKEDOWNS_PER_DAY`] today (or could not count: no secret).
    Server,
}

/// One UTC day's counts: the day (days since 1970), its secret, each mark's count, the total.
struct LimitDay {
    day: i64,
    secret: [u8; 32],
    by_address: HashMap<[u8; 16], u32>,
    total: u32,
}

/// The day's copyright-report counts (`RelayState::media_takedowns`), in memory only.
pub struct Limits {
    today: std::sync::Mutex<Option<LimitDay>>,
    per_address: u32,
    per_day: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Limits::with(TAKEDOWNS_PER_ADDRESS_PER_DAY, TAKEDOWNS_PER_DAY)
    }
}

impl std::fmt::Debug for Limits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the secret, never the marks.
        f.debug_struct("Limits").field("per_address", &self.per_address).field("per_day", &self.per_day).finish_non_exhaustive()
    }
}

impl Limits {
    /// Limits of `per_address` a day from one address and `per_day` from everyone (a test sets small ones).
    pub fn with(per_address: u32, per_day: u32) -> Self {
        Limits { today: std::sync::Mutex::new(None), per_address: per_address.max(1), per_day: per_day.max(1) }
    }

    /// Count one report from `address` at `now_ms` (Unix ms), or say which limit it ran into and
    /// count nothing. A new UTC day forgets every count and draws a new secret first.
    pub fn take(&self, address: &str, now_ms: i64) -> Result<(), Over> {
        let day = now_ms.div_euclid(DAY_MS);
        let mut today = self.today.lock().unwrap_or_else(|p| p.into_inner());
        if today.as_ref().is_none_or(|d| d.day != day) {
            let mut secret = [0u8; 32];
            if getrandom::getrandom(&mut secret).is_err() {
                // No secret, no count: refuse rather than keep counts anyone could work back to an address.
                tracing::error!("Copyright reports: no random secret for the day; none is taken in");
                return Err(Over::Server);
            }
            *today = Some(LimitDay { day, secret, by_address: HashMap::new(), total: 0 });
        }
        let Some(d) = today.as_mut() else { return Err(Over::Server) };
        let mark = mark_of(&d.secret, address);
        if d.by_address.get(&mark).copied().unwrap_or(0) >= self.per_address {
            return Err(Over::Address);
        }
        if d.total >= self.per_day {
            return Err(Over::Server);
        }
        *d.by_address.entry(mark).or_insert(0) += 1;
        d.total += 1;
        Ok(())
    }
}

/// The mark of `address` under the day's `secret`.
fn mark_of(secret: &[u8; 32], address: &str) -> [u8; 16] {
    let mut h = blake3::Hasher::new_keyed(secret);
    h.update(b"hum/takedown-limit/v1\n");
    h.update(address.as_bytes());
    let mut out = [0u8; 16];
    out.copy_from_slice(&h.finalize().as_bytes()[..16]);
    out
}

// ── POST /api/files/takedown ──

/// A form the route has read: the item's number and the person's words, cleaned.
#[derive(Debug, Clone, PartialEq)]
pub struct TakedownForm {
    pub item: i64,
    pub name: String,
    pub contact: String,
    pub statement: String,
}

/// One line of text: no control characters, the ends trimmed.
fn one_line(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect::<String>().trim().to_string()
}

/// A block of text: line breaks and tabs kept, every other control character dropped, the ends trimmed.
fn text_block(s: &str) -> String {
    s.chars().filter(|c| !c.is_control() || *c == '\n' || *c == '\t').collect::<String>().trim().to_string()
}

/// Read the form's JSON text: the box must be ticked (`good_faith` exactly `true`), a statement
/// given and every part within its length; `item` a positive whole number. Each refusal says what
/// to change.
pub fn read_form(text: &str) -> Result<TakedownForm, Refused> {
    let v: Value = serde_json::from_str(text).map_err(|_| bad(NOT_A_FORM))?;
    let Some(o) = v.as_object() else { return Err(bad(NOT_A_FORM)) };
    if o.get("good_faith") != Some(&Value::Bool(true)) {
        return Err(bad(NEEDS_GOOD_FAITH));
    }
    let field = |k: &str| o.get(k).and_then(Value::as_str).unwrap_or("");
    let statement = text_block(field("statement"));
    if statement.is_empty() {
        return Err(bad(NEEDS_STATEMENT));
    }
    if statement.chars().count() > STATEMENT_MAX {
        return Err(bad(format!("The statement is longer than {STATEMENT_MAX} characters. Shorten it, then send it again.")));
    }
    let name = one_line(field("name"));
    if name.chars().count() > NAME_MAX {
        return Err(bad(format!("The name is longer than {NAME_MAX} characters. Shorten it, then send it again.")));
    }
    let contact = one_line(field("contact"));
    if contact.chars().count() > CONTACT_MAX {
        return Err(bad(format!("The contact is longer than {CONTACT_MAX} characters. Shorten it, then send it again.")));
    }
    let item = o.get("item").and_then(Value::as_i64).filter(|n| *n > 0).ok_or_else(|| bad(NAME_THE_ITEM))?;
    Ok(TakedownForm { item, name, contact, statement })
}

/// `POST /api/files/takedown`: a copyright report from anyone, signed in or not.
pub async fn takedown(State(state): State<Arc<RelayState>>, headers: HeaderMap, text: String) -> Answer {
    takedown_core(&state, &headers, &text, now_ms()).await
}

/// [`takedown`] with the time named (the tests' way in). Read the form, find the item, count it
/// against the day's limits, keep it, and tell the admins and owners signed in now.
pub async fn takedown_core(state: &RelayState, headers: &HeaderMap, text: &str, now_ms: i64) -> Answer {
    let form = read_form(text)?;
    let item = state.db.media_item(form.item).map_err(db_err)?.ok_or_else(|| (StatusCode::NOT_FOUND, NO_SUCH_ITEM.to_string()))?;
    // Whether everyone can see the item now decides whether the public log may show its title.
    let public = shown_to_everyone(&item, &media_settings(&state.db));
    match state.media_takedowns.take(&address_of(headers), now_ms) {
        Ok(()) => {}
        Err(Over::Address) => return Err((StatusCode::TOO_MANY_REQUESTS, address_full_words())),
        Err(Over::Server) => return Err((StatusCode::TOO_MANY_REQUESTS, SERVER_FULL.to_string())),
    }
    let new = NewTakedown {
        item_id: item.id,
        item_title: &item.title,
        item_public: public,
        name: &form.name,
        contact: &form.contact,
        statement: &form.statement,
    };
    let id = state.db.takedown_add(&new, now_ms).map_err(|e| {
        // The error only: never what the person wrote.
        tracing::error!("Copyright reports: one could not be kept ({e})");
        (StatusCode::INTERNAL_SERVER_ERROR, "The server could not keep your report. Try again in a moment.".to_string())
    })?;
    tracing::info!("Copyright report #{id} received about Media item {}", item.id);
    let title = if item.title.trim().is_empty() { String::new() } else { format!(" \"{}\"", item.title.trim()) };
    tell_admins(
        state,
        &format!("New copyright report #{id} about item {}{title} on the Files page. Open its Admin tab to decide it.", item.id),
    )
    .await;
    Ok(Json(json!({ "ok": true, "id": id, "words": RECEIVED })))
}

/// A system line to every admin and owner signed in now (as handlers/reports.rs tells moderators).
async fn tell_admins(state: &RelayState, words: &str) {
    let admins: Vec<String> = state.peers.read().await.keys().filter(|k| is_admin(&state.db, k)).cloned().collect();
    for to in admins {
        let _ = state.broadcast_tx.send(RelayMessage::Private { to, message: words.to_string() });
    }
}

// ── GET /api/files/takedowns (the public log) ──

/// `GET /api/files/takedowns`: every decided copyright report, without who reported it.
pub async fn public_log(State(state): State<Arc<RelayState>>) -> Answer {
    log_core(&state.db)
}

/// The public log's answer: `{takedowns: [...]}`, the latest decision first.
pub fn log_core(db: &Storage) -> Answer {
    let rows = db.takedowns_decided(PUBLIC_LOG_MAX).map_err(db_err)?;
    Ok(Json(json!({ "takedowns": rows.iter().map(public_json).collect::<Vec<_>>() })))
}

/// One decided report as the public log shows it, built field by field: the item, the dates, the
/// outcome and the admin's note. NEVER the name, the contact, the statement or the admin's key.
/// A title only when everyone could see the item when it was reported.
pub fn public_json(r: &TakedownRow) -> Value {
    json!({
        "id": r.id,
        "item_id": r.item_id,
        "item_title": if r.item_public { r.item_title.as_str() } else { "" },
        "created_at": r.created_at,
        "decided_at": r.decided_at,
        "outcome": r.state,
        "note": r.decision_note,
    })
}

// ── GET or POST /api/files/takedowns/open (admins) ──

pub async fn open_list(State(state): State<Arc<RelayState>>, Query(b): Query<SignedOnly>) -> Answer {
    check_signed(&state, &b.key, b.timestamp, &b.sig, OPEN_PURPOSE).await?;
    open_core(&state.db, &b.key)
}

pub async fn open_list_post(State(state): State<Arc<RelayState>>, Json(b): Json<SignedOnly>) -> Answer {
    check_signed(&state, &b.key, b.timestamp, &b.sig, OPEN_PURPOSE).await?;
    open_core(&state.db, &b.key)
}

/// The open reports, with what the person wrote, to an admin or the owner only.
pub fn open_core(db: &Storage, key: &str) -> Answer {
    if !is_admin(db, key) {
        return Err(forbidden("Only an admin can see copyright reports."));
    }
    let rows = db.takedowns_open().map_err(db_err)?;
    let mut list = Vec::with_capacity(rows.len());
    for r in &rows {
        let on_page = db.media_item(r.item_id).map_err(db_err)?.is_some();
        list.push(json!({
            "id": r.id,
            "item_id": r.item_id,
            "item_title": r.item_title,
            "item_public": r.item_public,
            "item_on_page": on_page,
            "created_at": r.created_at,
            "name": r.name,
            "contact": r.contact,
            "statement": r.statement,
            "state": r.state,
        }));
    }
    Ok(Json(json!({ "requests": list })))
}

// ── POST /api/files/takedown/decide (admins) ──

/// What an admin decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The item is removed from the Files page.
    Removed,
    /// The item stays.
    Kept,
}

impl Outcome {
    pub fn word(self) -> &'static str {
        match self {
            Outcome::Removed => "removed",
            Outcome::Kept => "kept",
        }
    }

    pub fn from_word(w: &str) -> Option<Self> {
        match w {
            "removed" => Some(Outcome::Removed),
            "kept" => Some(Outcome::Kept),
            _ => None,
        }
    }
}

/// The purpose a decision is signed over: `media_takedown:<id>:<removed|kept>`.
pub fn decide_purpose(id: i64, outcome: Outcome) -> String {
    format!("media_takedown:{id}:{}", outcome.word())
}

#[derive(Debug, Default, Deserialize)]
pub struct DecideBody {
    pub key: String,
    pub timestamp: u64,
    pub sig: String,
    pub id: i64,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub note: String,
}

pub async fn decide(State(state): State<Arc<RelayState>>, Json(b): Json<DecideBody>) -> Answer {
    let outcome = Outcome::from_word(&b.state).ok_or_else(|| bad("Decide a copyright report as \"removed\" or \"kept\"."))?;
    check_signed(&state, &b.key, b.timestamp, &b.sig, &decide_purpose(b.id, outcome)).await?;
    decide_core(&state.db, Folders::live(), &b.key, b.id, outcome, &b.note, now_ms())
}

/// One decision at a time, so two admins deciding the same report never both act on it.
static DECIDING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// An admin or the owner decides open report `id`: `Removed` removes its item through the Media
/// page's remove path (an item already gone counts as removed), `Kept` leaves it; either way the
/// decision and the note go into the public log.
pub fn decide_core(db: &Storage, folders: Folders, key: &str, id: i64, outcome: Outcome, note: &str, now_ms: i64) -> Answer {
    if !is_admin(db, key) {
        return Err(forbidden("Only an admin can decide copyright reports."));
    }
    let note = text_block(note);
    if note.chars().count() > NOTE_MAX {
        return Err(bad(format!("The note is longer than {NOTE_MAX} characters. Shorten it, then decide again.")));
    }
    let _one = DECIDING.lock().unwrap_or_else(|p| p.into_inner());
    let row = db
        .takedown_by_id(id)
        .map_err(db_err)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "There is no copyright report with that number.".to_string()))?;
    if row.state != "open" {
        return Err((StatusCode::CONFLICT, format!("Copyright report #{id} was already decided ({}).", row.state)));
    }
    let mut removal = Value::Null;
    let mut already_gone = false;
    if outcome == Outcome::Removed {
        match db.media_item(row.item_id).map_err(db_err)? {
            None => already_gone = true,
            Some(_) => removal = remove_core(db, folders, &RemoveTarget::Item(row.item_id), key)?.0,
        }
    }
    if !db.takedown_decide(id, outcome.word(), &note, key, now_ms).map_err(db_err)? {
        return Err((StatusCode::CONFLICT, format!("Copyright report #{id} was already decided.")));
    }
    tracing::info!("Copyright report #{id} decided: {} (Media item {})", outcome.word(), row.item_id);
    Ok(Json(json!({
        "ok": true,
        "id": id,
        "outcome": outcome.word(),
        "item_id": row.item_id,
        "item_removed": !removal.is_null(),
        "item_already_gone": already_gone,
        "removal": removal,
    })))
}

#[cfg(test)]
#[path = "files_takedown_tests.rs"]
mod tests;
