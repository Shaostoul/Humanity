//! "Report a copyright problem" and the public takedown log on the desktop app (2026-10-13). The
//! website's half is web/pages/files-takedown.js, whose words and behaviour this file copies (web
//! mirrors native, but here the website came first: both halves follow the operator's answers of
//! 2026-10-13, "a simple form so it's easy... a dedicated button somewhere on the media page... We
//! can log the reports publicly"). The relay half is src/relay/files_takedown.rs: its header holds
//! the routes, the bodies and the answers built here.
//!
//! It holds three things:
//!
//! - THE FORM ([`Form`], drawn by [`draw_windows`]): opened with "Report a copyright problem" from
//!   an item's own page (about that item) and from the foot of the Files page (where it asks for the
//!   item's number or address). It works for anyone, signed in or not, a member or not: it sends
//!   `POST /api/files/takedown` with `{item, name, contact, statement, good_faith}`, unsigned and
//!   with no upload token, and says in one sentence what happens next ([`NEXT`]). Send stays off
//!   until the good-faith box is ticked; the words beside the box are the relay's own
//!   [`GOOD_FAITH_LINE`], never a copy. A refusal shows the relay's own words.
//! - THE ADMINS' SECTION ([`draw_admin_section`], on the Files page's Admin tab, files/admin.rs):
//!   the reports still open (`POST /api/files/takedowns/open`, signed `media_takedowns`), each with
//!   what the person wrote, a note for the public log, and Remove item (which asks first) or Keep
//!   (`POST /api/files/takedown/decide`, signed `media_takedown:<id>:<removed|kept>`). Drawn, and
//!   asked for, only for an admin of this server.
//! - THE PUBLIC LOG ([`LogRow`], opened from the Files page's foot, the form once it went, and the
//!   admins' section): `GET /api/files/takedowns`, shown in a window. It shows the item, the dates,
//!   the outcome and the admin's note. The relay never sends who reported or how to reach them, and
//!   [`parse_log`] reads only the fields the window shows, so nothing else could reach it even from
//!   a server that sent more.
//!
//! Every request is a Files page job (pages/files.rs `Job`, `spawn`): built on the frame, sent on a
//! worker thread, its answer read on a later frame by [`on_answer`]. Nothing here logs what a person
//! wrote: the job log names the route only.

use std::collections::HashMap;

use egui::RichText;
use serde_json::{json, Value};

use super::api::{self, Method, Refusal, Request};
use super::Job;
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

/// The words beside the box the form must have ticked: the relay's own, so the two never drift.
pub use crate::relay::files_takedown::GOOD_FAITH_LINE;
/// The relay's limits, in characters (the text boxes stop there).
use crate::relay::files_takedown::{CONTACT_MAX, NAME_MAX, NOTE_MAX, OPEN_PURPOSE, STATEMENT_MAX};

// ── The words (files-takedown.js WORDS, word for word) ─────────────────────────────────────────

pub const BUTTON: &str = "Report a copyright problem";
pub const BUTTON_TIP: &str = "Is this your work, put here without your permission? Tell the admins.";
pub const TITLE: &str = "Report a copyright problem";
pub const LEAD: &str =
    "If something on this Files page is your work and it was put here without your permission, tell this server's admins. You do not need to be signed in.";
/// The one sentence on what happens next (the operator's design, 2026-10-13).
pub const NEXT: &str =
    "An admin looks at it and removes the item or keeps it; the outcome appears in the public takedown log, without your name or contact.";
pub const ITEM_LABEL: &str = "The item's number or address";
pub const ITEM_HINT: &str = "For example 12, or united-humanity.us/files/12.";
pub const NAME_LABEL: &str = "Your name (optional)";
pub const CONTACT_LABEL: &str = "How to reach you (optional; only the admins see it)";
pub const STATEMENT_LABEL: &str = "Why the work is yours";
pub const STATEMENT_HINT: &str = "For example: where and when you first published it.";
pub const SEND: &str = "Send";
pub const SENDING: &str = "Sending...";
pub const CANCEL: &str = "Cancel";
pub const CLOSE: &str = "Close";
pub const NEED_ITEM: &str = "Give the item's number, or paste its address.";
pub const NEED_STATEMENT: &str = "Say why the work is yours.";
pub const NEED_BOX: &str = "Tick the box first.";
/// The link to the public log (the website's Files page foot and form say the same).
pub const LOG_LINK: &str = "The public takedown log";
pub const LOG_TITLE: &str = "The public takedown log";
/// What the log window says above the list (the website's /rules page says it around its log).
pub const LOG_LEAD: &str =
    "Every copyright report this server has decided, the latest first. Who reported it, and how to reach them, is never shown.";
pub const LOG_EMPTY: &str = "No copyright report has been decided yet.";
pub const LOG_FAILED: &str = "The takedown log could not be loaded: ";
pub const REMOVED: &str = "Removed";
pub const KEPT: &str = "Kept";
/// The admins' section's heading (the website's Admin tab panel says the same).
pub const ADMIN_HEADING: &str = "Copyright reports";
pub const ADMIN_LEAD: &str =
    "Reports sent with Report a copyright problem. Your decision and your note go into the public takedown log; the person's name, contact and statement never do.";
pub const ADMIN_NONE: &str = "No copyright reports are waiting.";
pub const ADMIN_NOTE_HINT: &str = "A note for the public log (optional). Do not put the person's name or contact in it.";
pub const REMOVE_ITEM: &str = "Remove item";
pub const KEEP: &str = "Keep";
pub const GONE: &str = "(no longer on the Files page)";
pub const REMOVE_CONFIRM: &str = "Remove this item from the Files page? The decision and your note go into the public takedown log.";
/// The confirmation's own buttons (the prune lists' words, files/admin.rs).
pub const YES_REMOVE: &str = "Yes, remove";
/// What the section says to an admin who is not signed in to this server (nothing can be signed).
pub const SIGN_IN_FIRST: &str = "Sign in to this server to see its copyright reports.";

// ── The relay's routes ─────────────────────────────────────────────────────────────────────────

pub const TAKEDOWN_PATH: &str = "/api/files/takedown";
pub const LOG_PATH: &str = "/api/files/takedowns";
pub const OPEN_PATH: &str = "/api/files/takedowns/open";
pub const DECIDE_PATH: &str = "/api/files/takedown/decide";

// ── What the page holds ────────────────────────────────────────────────────────────────────────

/// The form, while it is open.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Form {
    /// This form's number (media.rs `next_id`): its answer finds it by this, and an answer for a
    /// form closed meanwhile is dropped.
    pub who: u64,
    /// The item's number: given when opened from an item's page, else read from `item_text`.
    pub item: Option<i64>,
    /// The item's title as its page shows it ('' when opened from the foot).
    pub title: String,
    /// Opened from an item's page: the item is named, not asked for.
    pub fixed: bool,
    /// What was typed into the item box (opened from the foot).
    pub item_text: String,
    pub name: String,
    pub contact: String,
    pub statement: String,
    /// The good-faith box. Send stays off until it is ticked.
    pub good_faith: bool,
    pub sending: bool,
    /// What is wrong, or the relay's refusal, in its own words.
    pub error: String,
    /// The relay's words once the report was kept.
    pub done: String,
}

/// One decided report as the public log shows it: the item, the dates, the outcome and the note.
/// There is no field for who reported it or how to reach them, so nothing can show them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LogRow {
    pub id: i64,
    pub item_id: i64,
    /// '' when the item was not public when it was reported (the relay leaves it out then).
    pub item_title: String,
    pub created_at: i64,
    pub decided_at: i64,
    /// "removed" or "kept".
    pub outcome: String,
    pub note: String,
}

/// The log window, while it is open.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LogView {
    /// None until the server answered.
    pub rows: Option<Vec<LogRow>>,
    pub error: String,
}

/// One report still open, as an admin reads it (with what the person wrote).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OpenReport {
    pub id: i64,
    pub item_id: i64,
    pub item_title: String,
    pub item_public: bool,
    /// The item is still on the Files page (an item already gone counts as removed).
    pub item_on_page: bool,
    pub created_at: i64,
    pub name: String,
    pub contact: String,
    pub statement: String,
    pub state: String,
}

/// Everything "Report a copyright problem" keeps on the Files page (`FilesState::takedown`). It
/// belongs to one server and one person, like the rest of the page's state (media.rs `sync_server`).
#[derive(Debug, Clone, Default)]
pub struct Takedowns {
    pub form: Option<Form>,
    pub log: Option<LogView>,
    /// The admins' section asked for the open reports (once per server and person; Refresh asks again).
    pub requested: bool,
    /// The open reports; None until the server answered.
    pub open: Option<Vec<OpenReport>>,
    /// Each open report's note for the public log, by report number, as the admin types it.
    pub notes: HashMap<i64, String>,
    /// A Remove item pressed, waiting for its confirmation (the report's number).
    pub confirm_remove: Option<i64>,
    /// One line about the last decision, or a refusal; `warn` colours it.
    pub status: String,
    pub warn: bool,
}

// ── Reading what was typed, and building the requests ─────────────────────────────────────────

/// The item's number from what a person typed (files-takedown.js `itemNumberFrom`): "12", "#12",
/// "/files/12" or a whole address ending in /files/12 (a query or a fragment after it is fine).
/// None when it names none.
pub fn item_number_from(text: &str) -> Option<i64> {
    let t = text.trim();
    // At most 15 digits, as the website reads it: a number that safely fits.
    let digits = |s: &str| -> Option<i64> {
        if s.is_empty() || s.len() > 15 || !s.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        s.parse::<i64>().ok().filter(|n| *n > 0)
    };
    if let Some(n) = digits(t.strip_prefix('#').unwrap_or(t)) {
        return Some(n);
    }
    // An address: cut a query or a fragment off its end, and one trailing slash.
    let cut = t.find(|c: char| c == '?' || c == '#').map(|i| &t[..i]).unwrap_or(t);
    let cut = cut.strip_suffix('/').unwrap_or(cut);
    let at = cut.rfind("/files/")?;
    digits(&cut[at + "/files/".len()..])
}

/// What is missing from a form before it can be sent, in words (files-takedown.js `formProblem`),
/// or None when nothing is.
pub fn form_problem(f: &Form) -> Option<&'static str> {
    if f.item.is_none() {
        return Some(NEED_ITEM);
    }
    if f.statement.trim().is_empty() {
        return Some(NEED_STATEMENT);
    }
    if !f.good_faith {
        return Some(NEED_BOX);
    }
    None
}

/// The body the relay takes (files-takedown.js `takedownBody`): `{item, name, contact, statement,
/// good_faith}`, the words trimmed. Unsigned: anyone may send it.
pub fn send_request(f: &Form) -> Request {
    let body = json!({
        "item": f.item.unwrap_or(0),
        "name": f.name.trim(),
        "contact": f.contact.trim(),
        "statement": f.statement.trim(),
        "good_faith": f.good_faith,
    });
    Request { method: Method::Post, path: TAKEDOWN_PATH.to_string(), body: Some(body.to_string()), purpose: None }
}

/// The public log: an unsigned read anyone may make.
pub fn log_request() -> Request {
    Request { method: Method::Get, path: LOG_PATH.to_string(), body: None, purpose: None }
}

/// The open reports, for an admin: a signed read, in a POST body (a signature in an address is
/// about 10 KB, which a proxy refuses).
pub fn open_request(signer: &api::Signer) -> Request {
    api::post(OPEN_PATH, OPEN_PURPOSE.to_string(), signer.signed(OPEN_PURPOSE, json!({})))
}

/// The word a decision is sent and signed with: "removed" or "kept".
pub fn decide_word(removed: bool) -> &'static str {
    if removed {
        "removed"
    } else {
        "kept"
    }
}

/// The purpose a decision is signed over (the relay's `decide_purpose`):
/// `media_takedown:<id>:<removed|kept>`.
pub fn decide_purpose(id: i64, removed: bool) -> String {
    format!("media_takedown:{id}:{}", decide_word(removed))
}

/// An admin's decision on report `id`, with the note for the public log (trimmed).
pub fn decide_request(signer: &api::Signer, id: i64, removed: bool, note: &str) -> Request {
    let purpose = decide_purpose(id, removed);
    let body = json!({ "id": id, "state": decide_word(removed), "note": note.trim() });
    api::post(DECIDE_PATH, purpose.clone(), signer.signed(&purpose, body))
}

// ── Reading the answers ─────────────────────────────────────────────────────────────────────────

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

fn i(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(Value::as_i64).unwrap_or(0)
}

/// The public log's answer, `{takedowns: [...]}`. Only the fields the window shows are read, so a
/// name, a contact or a statement could never reach it, whatever a server sent.
pub fn parse_log(v: &Value) -> Vec<LogRow> {
    v.get("takedowns")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|r| LogRow {
                    id: i(r, "id"),
                    item_id: i(r, "item_id"),
                    item_title: s(r, "item_title"),
                    created_at: i(r, "created_at"),
                    decided_at: i(r, "decided_at"),
                    outcome: s(r, "outcome"),
                    note: s(r, "note"),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The open reports' answer, `{requests: [...]}` (an admin's read).
pub fn parse_open(v: &Value) -> Vec<OpenReport> {
    v.get("requests")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|r| {
                    Some(OpenReport {
                        id: r.get("id")?.as_i64()?,
                        item_id: i(r, "item_id"),
                        item_title: s(r, "item_title"),
                        item_public: r.get("item_public").and_then(Value::as_bool).unwrap_or(false),
                        // Not said: taken as still there (the website reads only an explicit false).
                        item_on_page: r.get("item_on_page").and_then(Value::as_bool).unwrap_or(true),
                        created_at: i(r, "created_at"),
                        name: s(r, "name"),
                        contact: s(r, "contact"),
                        statement: s(r, "statement"),
                        state: s(r, "state"),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The day of a Unix-ms time, as 2026-10-13 (UTC), or '' when there is none.
pub fn day_of(ms: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    crate::gui::pages::game_admin::format_ban_date(ms).chars().take(10).collect()
}

/// How the log names an item (files-takedown.js `itemWords`): its title in quotes, or "Item 12"
/// when it shows none.
pub fn item_words(id: i64, title: &str) -> String {
    let t = title.trim();
    if t.is_empty() {
        format!("Item {id}")
    } else {
        format!("Item {id}, \"{t}\"")
    }
}

// ── Opening things, and the answers ────────────────────────────────────────────────────────────

/// Open the form: about `item` (from its page), or asking for the item's number (from the foot).
pub(crate) fn open_form(state: &mut GuiState, item: Option<&api::Item>) {
    let who = super::next_id();
    state.files.takedown.form = Some(match item {
        Some(it) => Form { who, item: Some(it.id), title: it.display_title(), fixed: true, ..Default::default() },
        None => Form { who, ..Default::default() },
    });
}

/// Open the log window and ask the server for the log (again, each time it opens: it is read from
/// the server each time, as the website's /rules page reads it).
pub(crate) fn open_log(state: &mut GuiState) {
    state.files.takedown.log = Some(LogView::default());
    if super::server_here(state).is_empty() {
        if let Some(log) = state.files.takedown.log.as_mut() {
            log.error = format!("{LOG_FAILED}there is no server to ask.");
        }
        return;
    }
    super::spawn(state, Job::TakedownLog, log_request());
}

/// Ask for the open reports (an admin signed in here), unless that is already on its way.
pub(crate) fn refresh_open(state: &mut GuiState) {
    if state.files.jobs.iter().any(|(j, _)| *j == Job::TakedownOpen) {
        return;
    }
    let Some(signer) = super::signer_here(state) else { return };
    state.files.takedown.requested = true;
    super::spawn(state, Job::TakedownOpen, open_request(&signer));
}

/// An answer for "Report a copyright problem" (pages/files.rs `on_done` hands these over).
pub(crate) fn on_answer(state: &mut GuiState, job: Job, result: Result<Value, Refusal>) {
    let t = &mut state.files.takedown;
    match (job, result) {
        (Job::TakedownSend { who }, result) => {
            // A form closed (or another opened) meanwhile: nothing to show it in.
            let Some(f) = t.form.as_mut().filter(|f| f.who == who) else { return };
            f.sending = false;
            match result {
                Ok(v) => {
                    let words = v.get("words").and_then(Value::as_str).filter(|w| !w.trim().is_empty()).unwrap_or("Received.");
                    f.done = words.to_string();
                    f.error.clear();
                }
                // The relay's own words: what to change, or which limit was reached.
                Err(e) => f.error = e.words,
            }
        }
        (Job::TakedownLog, result) => {
            let Some(log) = t.log.as_mut() else { return };
            match result {
                Ok(v) => {
                    log.rows = Some(parse_log(&v));
                    log.error.clear();
                }
                Err(e) => log.error = format!("{LOG_FAILED}{}", e.words),
            }
        }
        (Job::TakedownOpen, Ok(v)) => t.open = Some(parse_open(&v)),
        (Job::TakedownOpen, Err(e)) => {
            t.open.get_or_insert_with(Vec::new);
            t.status = format!("The copyright reports could not be loaded: {}", e.words);
            t.warn = true;
        }
        (Job::TakedownDecide { id, removed }, Ok(v)) => {
            let what = if !removed {
                "the item was kept."
            } else if v.get("item_already_gone").and_then(Value::as_bool).unwrap_or(false) {
                "the item was already gone; recorded as removed."
            } else {
                "the item was removed."
            };
            t.status = format!("Report #{id}: {what} It is in the public takedown log.");
            t.warn = false;
            t.notes.remove(&id);
            if removed {
                // The Media list changed: load it again, and the admins' meter with it.
                super::reload(state);
                if state.files.admin.requested {
                    super::refresh_usage(state);
                }
            }
            refresh_open(state);
        }
        (Job::TakedownDecide { .. }, Err(e)) => {
            t.status = e.words;
            t.warn = true;
            refresh_open(state);
        }
        (other, _) => log::warn!("Files: {other:?} is not a copyright report's answer"),
    }
}

// ── Drawing ────────────────────────────────────────────────────────────────────────────────────

/// "Report a copyright problem" in an item's own row of actions (item_view.rs `draw_actions`): for
/// anyone, signed in or not. True when pressed.
pub(crate) fn item_button(ui: &mut egui::Ui, theme: &Theme) -> bool {
    widgets::Button::ghost(BUTTON).size(widgets::ButtonSize::Small).tooltip(BUTTON_TIP).show(ui, theme)
}

/// The Files page's foot (the list tab): "Report a copyright problem" (the form asks for the item)
/// and the link to the public log, as the website's page foot has them.
pub(crate) fn draw_foot(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.add_space(theme.spacing_md);
    ui.horizontal_wrapped(|ui| {
        if widgets::Button::secondary(BUTTON).size(widgets::ButtonSize::Small).tooltip(BUTTON_TIP).show(ui, theme) {
            open_form(state, None);
        }
        // A ghost button is the house inline link (widgets/button.rs), with a hit area of its own.
        if widgets::Button::ghost(LOG_LINK).size(widgets::ButtonSize::Small).tooltip(LOG_LEAD).show(ui, theme) {
            open_log(state);
        }
    });
}

/// The form and the log window, while open. Drawn after the Files page and an item's page, so they
/// sit over both (pages/files.rs `draw`).
pub(crate) fn draw_windows(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    draw_form(ctx, theme, state);
    draw_log(ctx, theme, state);
}

/// A small grey label above a field.
fn field_label(ui: &mut egui::Ui, theme: &Theme, text: &str) {
    ui.label(RichText::new(text).size(theme.font_size_small).color(theme.text_secondary()));
}

fn draw_form(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let Some(mut f) = state.files.takedown.form.take() else { return };
    let mut open = true;
    let mut send = false;
    let mut close = false;
    let mut show_log = false;
    widgets::dialog_scrolling(ctx, theme, "media_takedown_form", TITLE, &mut open, |ui| {
        ui.set_max_width(520.0);
        if !f.done.is_empty() {
            // Sent: the relay's own words, the log, and Close.
            ui.label(RichText::new(&f.done).size(theme.font_size_body).color(theme.success()));
            ui.add_space(theme.spacing_xs);
            show_log = widgets::Button::ghost(LOG_LINK).size(widgets::ButtonSize::Small).show(ui, theme);
            ui.add_space(theme.spacing_sm);
            close = widgets::Button::primary(CLOSE).show(ui, theme);
            return;
        }
        widgets::body_hint(ui, theme, LEAD);
        ui.label(RichText::new(NEXT).size(theme.font_size_small).color(theme.text_secondary()));
        ui.add_space(theme.spacing_sm);
        if f.fixed {
            let about = format!("About: {}", item_words(f.item.unwrap_or(0), &f.title));
            ui.label(RichText::new(about).size(theme.font_size_body).color(theme.text_primary()).strong());
        } else {
            field_label(ui, theme, ITEM_LABEL);
            if ui.add(egui::TextEdit::singleline(&mut f.item_text).desired_width(480.0).char_limit(300)).changed() {
                f.item = item_number_from(&f.item_text);
            }
            widgets::body_hint(ui, theme, ITEM_HINT);
        }
        ui.add_space(theme.spacing_xs);
        field_label(ui, theme, NAME_LABEL);
        ui.add(egui::TextEdit::singleline(&mut f.name).desired_width(480.0).char_limit(NAME_MAX));
        field_label(ui, theme, CONTACT_LABEL);
        ui.add(egui::TextEdit::singleline(&mut f.contact).desired_width(480.0).char_limit(CONTACT_MAX));
        field_label(ui, theme, STATEMENT_LABEL);
        ui.add(egui::TextEdit::multiline(&mut f.statement).desired_rows(5).desired_width(480.0).hint_text(STATEMENT_HINT).char_limit(STATEMENT_MAX));
        ui.add_space(theme.spacing_xs);
        // The relay's own words beside the box (GOOD_FAITH_LINE).
        ui.checkbox(&mut f.good_faith, GOOD_FAITH_LINE);
        if !f.error.is_empty() {
            ui.label(RichText::new(&f.error).size(theme.font_size_small).color(theme.danger()));
        }
        ui.add_space(theme.spacing_sm);
        ui.horizontal(|ui| {
            // Off until the box is ticked, and while it is on its way.
            send = widgets::Button::danger(if f.sending { SENDING } else { SEND }).disabled(f.sending || !f.good_faith).show(ui, theme);
            close = widgets::Button::secondary(CANCEL).show(ui, theme);
        });
    });
    if show_log {
        open_log(state);
    }
    if !open || close {
        // Closed: an answer still on its way finds no form and is dropped.
        return;
    }
    if send && !f.sending {
        match form_problem(&f) {
            Some(problem) => f.error = problem.to_string(),
            None => {
                f.sending = true;
                f.error.clear();
                let req = send_request(&f);
                super::spawn(state, Job::TakedownSend { who: f.who }, req);
            }
        }
    }
    state.files.takedown.form = Some(f);
}

fn draw_log(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let Some(log) = state.files.takedown.log.clone() else { return };
    let mut open = true;
    widgets::dialog_scrolling(ctx, theme, "media_takedown_log", LOG_TITLE, &mut open, |ui| {
        ui.set_max_width(640.0);
        widgets::body_hint(ui, theme, LOG_LEAD);
        ui.add_space(theme.spacing_sm);
        if !log.error.is_empty() {
            ui.label(RichText::new(&log.error).size(theme.font_size_small).color(theme.danger()));
            return;
        }
        let Some(rows) = log.rows.as_ref() else {
            ui.label(RichText::new("Loading...").size(theme.font_size_small).color(theme.text_muted()));
            return;
        };
        if rows.is_empty() {
            ui.label(RichText::new(LOG_EMPTY).size(theme.font_size_body).color(theme.text_muted()));
            return;
        }
        for r in rows {
            draw_log_row(ui, theme, r);
            ui.add_space(theme.spacing_xs);
        }
    });
    if !open {
        state.files.takedown.log = None;
    }
}

/// One decided report: its outcome, the item, the dates and number, and the admin's note. Only the
/// [`LogRow`] fields exist to be drawn.
fn draw_log_row(ui: &mut egui::Ui, theme: &Theme, r: &LogRow) {
    egui::Frame::none().stroke(egui::Stroke::new(1.0, theme.border())).rounding(egui::Rounding::same(theme.border_radius as u8)).inner_margin(theme.spacing_sm).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            let (word, color) = match r.outcome.as_str() {
                "removed" => (REMOVED, theme.danger()),
                "kept" => (KEPT, theme.accent()),
                _ => ("", theme.text_muted()),
            };
            if !word.is_empty() {
                ui.label(RichText::new(word).size(theme.font_size_body).color(color).strong());
            }
            ui.label(RichText::new(item_words(r.item_id, &r.item_title)).size(theme.font_size_body).color(theme.text_primary()));
            let created = day_of(r.created_at);
            let decided = day_of(r.decided_at);
            let when = format!(
                "· Reported {}, decided {} · #{}",
                if created.is_empty() { "on an unknown day" } else { created.as_str() },
                if decided.is_empty() { "on an unknown day" } else { decided.as_str() },
                r.id
            );
            ui.label(RichText::new(when).size(theme.font_size_small).color(theme.text_muted()));
        });
        if !r.note.trim().is_empty() {
            ui.label(RichText::new(r.note.trim()).size(theme.font_size_small).color(theme.text_secondary()));
        }
    });
}

/// The copyright reports on the Files page's Admin tab (files/admin.rs `draw_admin`): for an admin
/// of this server only. Anyone else gets nothing drawn and nothing asked, whichever way they reached
/// the tab.
pub(crate) fn draw_admin_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    if !super::is_admin(state) {
        return;
    }
    ui.horizontal(|ui| {
        ui.label(RichText::new(ADMIN_HEADING).size(theme.font_size_body).color(theme.text_primary()).strong());
        if widgets::Button::ghost("Refresh").size(widgets::ButtonSize::Small).show(ui, theme) {
            refresh_open(state);
        }
    });
    widgets::body_hint(ui, theme, ADMIN_LEAD);
    if super::signer_here(state).is_none() {
        widgets::body_hint(ui, theme, SIGN_IN_FIRST);
        return;
    }
    if !state.files.takedown.requested {
        refresh_open(state);
    }
    let reports = state.files.takedown.open.clone();
    match reports.as_deref() {
        None => {
            ui.label(RichText::new("Loading...").size(theme.font_size_small).color(theme.text_muted()));
        }
        Some([]) => widgets::body_hint(ui, theme, ADMIN_NONE),
        Some(list) => {
            for r in list {
                draw_open_report(ui, theme, state, r);
                ui.add_space(theme.spacing_xs);
            }
        }
    }
    let t = &state.files.takedown;
    if !t.status.is_empty() {
        ui.label(RichText::new(&t.status).size(theme.font_size_small).color(if t.warn { theme.warning() } else { theme.text_secondary() }));
    }
    if widgets::Button::ghost(LOG_LINK).size(widgets::ButtonSize::Small).show(ui, theme) {
        open_log(state);
    }
}

/// One open report: the item, when it came, what the person wrote, the note for the public log, and
/// Remove item (which asks first) and Keep.
fn draw_open_report(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, r: &OpenReport) {
    let busy = state.files.jobs.iter().any(|(j, _)| matches!(j, Job::TakedownDecide { id, .. } if *id == r.id));
    let asking = state.files.takedown.confirm_remove == Some(r.id);
    let mut decide: Option<bool> = None;
    let mut open_item = false;
    egui::Frame::none().stroke(egui::Stroke::new(1.0, theme.border())).rounding(egui::Rounding::same(theme.border_radius as u8)).inner_margin(theme.spacing_sm).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            let mut head = format!("Report #{} about {}", r.id, item_words(r.item_id, &r.item_title));
            if !r.item_on_page {
                head = format!("{head} {GONE}");
            }
            ui.label(RichText::new(head).size(theme.font_size_body).color(theme.text_primary()).strong());
            if r.item_on_page && widgets::Button::ghost("Open the item").size(widgets::ButtonSize::Small).show(ui, theme) {
                open_item = true;
            }
        });
        let received = day_of(r.created_at);
        let received = if received.is_empty() { "on an unknown day".to_string() } else { received };
        let given = |s: &str| if s.trim().is_empty() { "not given".to_string() } else { s.trim().to_string() };
        for line in [format!("Received {received}"), format!("Name: {}", given(&r.name)), format!("Contact: {}", given(&r.contact))] {
            ui.label(RichText::new(line).size(theme.font_size_small).color(theme.text_secondary()));
        }
        ui.label(RichText::new(r.statement.trim()).size(theme.font_size_body).color(theme.text_primary()));
        let note = state.files.takedown.notes.entry(r.id).or_default();
        ui.add(egui::TextEdit::multiline(note).desired_rows(2).desired_width(f32::INFINITY).hint_text(ADMIN_NOTE_HINT).char_limit(NOTE_MAX));
        let small = widgets::ButtonSize::Small;
        ui.horizontal_wrapped(|ui| {
            if asking {
                ui.label(RichText::new(REMOVE_CONFIRM).size(theme.font_size_small).color(theme.warning()));
                if widgets::Button::danger(YES_REMOVE).size(small).disabled(busy).show(ui, theme) {
                    decide = Some(true);
                }
                if widgets::Button::secondary(CANCEL).size(small).show(ui, theme) {
                    state.files.takedown.confirm_remove = None;
                }
                return;
            }
            if widgets::Button::danger(REMOVE_ITEM).size(small).disabled(busy).show(ui, theme) {
                // Asks first, as the website's confirm does.
                state.files.takedown.confirm_remove = Some(r.id);
            }
            if widgets::Button::secondary(KEEP).size(small).disabled(busy).show(ui, theme) {
                decide = Some(false);
            }
        });
    });
    if open_item {
        super::open_item_by_number(state, r.item_id);
    }
    if let Some(removed) = decide {
        state.files.takedown.confirm_remove = None;
        let Some(signer) = super::signer_or_say(state) else { return };
        let note = state.files.takedown.notes.get(&r.id).cloned().unwrap_or_default();
        super::spawn(state, Job::TakedownDecide { id: r.id, removed }, decide_request(&signer, r.id, removed, &note));
    }
}

#[cfg(test)]
#[path = "takedown_tests.rs"]
mod tests;
