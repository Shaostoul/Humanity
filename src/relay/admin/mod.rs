//! The admin action registry, and the two endpoints an app or an AI uses it through
//! (2026-10-22; docs/design/in-app-ops.md, "North star: the admin action registry").
//!
//! WHAT IT IS. Every action a server's admins and moderators can take is an entry in
//! `data/admin/actions.json`: its id, label, description, the role it needs, the inputs it takes,
//! how dangerous it is, and how it runs. This module builds that file into the relay
//! ([`REGISTRY_JSON`]), so what a server will run is what its own build says, never a file edited
//! on its disk after the fact.
//!
//! THE ENDPOINTS.
//! - `GET /api/admin/actions` ([`get_actions`]): the registry, to anyone. It is the same data as
//!   the file in the open-source repository and holds nothing secret, and serving it openly lets
//!   an app or an AI learn what a server can do before it holds an admin key.
//! - `POST /api/admin/actions/run` ([`post_run`]): run one action whose entry says
//!   `"run": "here"`. Signed with the caller's Dilithium3 key over [`run_purpose`], which names
//!   the action and the BLAKE3 of the exact inputs text, so a signature runs that action with
//!   those inputs and nothing else; fresh (5 minutes), used once, never a bare token. Then the
//!   role the entry asks for, then the confirm word of a destructive action, then the inputs
//!   checked against the entry, then the runner. Every change is written to the admin log
//!   (storage/admin_log.rs).
//!
//! THE RUNNERS. [`RUNNERS`] pairs each `"run": "here"` id with the function that carries it out.
//! `tests.rs` holds the two directions apart from each other: an entry without a runner, or a
//! runner without an entry, fails the build's tests. Actions that run as a chat command or on a
//! route of their own keep running there; their entries are what make them listable.
//!
//! RESERVED. `backups.` belongs to the Backups panel (continuous backup, `backup_stream/`). Its
//! actions are listed under that prefix and still run at its own `POST /api/admin/backups`;
//! moving one here is its entry's `"run": "here"` plus a line in [`RUNNERS`].

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::relay::relay::RelayState;

pub mod alerts;
pub mod federation;
pub mod members;
#[cfg(test)]
mod tests;

/// The registry, as built into this relay.
pub const REGISTRY_JSON: &str = include_str!("../../../data/admin/actions.json");

/// The purpose every run signs starts with this.
pub const RUN_PURPOSE_PREFIX: &str = "admin_action";

/// The tag a run's signature is remembered under for the used-once check.
const RUN_NONCE_TAG: &str = "admin_action";

/// The whole registry file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registry {
    #[serde(default)]
    pub signing: serde_json::Value,
    #[serde(default)]
    pub roles: BTreeMap<String, String>,
    #[serde(default)]
    pub danger_levels: BTreeMap<String, String>,
    #[serde(default)]
    pub reserved: Vec<Reserved>,
    #[serde(default)]
    pub categories: Vec<Category>,
    #[serde(default)]
    pub actions: Vec<ActionDef>,
}

/// A prefix of action ids kept for work built elsewhere.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reserved {
    pub prefix: String,
    #[serde(rename = "for", default)]
    pub for_whom: String,
}

/// A heading the actions are grouped under.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Category {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: String,
}

/// One admin action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionDef {
    /// `<category>.<verb>`, e.g. `alerts.add`.
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: String,
    pub category: String,
    /// The role it needs: `admin`, `mod` or (the claim only) `member`.
    pub role: String,
    /// `read`, `safe`, `caution` or `destructive` (see the file's `danger_levels`).
    pub danger: String,
    /// What a person types to run a destructive action (and an AI sends as `confirm`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirm_word: Option<String>,
    #[serde(default)]
    pub inputs: Vec<InputDef>,
    /// `"here"`: runs through `POST /api/admin/actions/run`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    /// The chat command that does the same, its inputs in braces: `/server-add {url} {name}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat: Option<String>,
    /// A signed endpoint of its own: `POST /api/admin/stats`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
}

/// One input an action takes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InputDef {
    pub name: String,
    pub label: String,
    /// `text`, `secret`, `number`, `bool` or `choice`.
    pub kind: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_len: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<i64>,
}

/// The input kinds the apps know how to draw.
pub const INPUT_KINDS: &[&str] = &["text", "secret", "number", "bool", "choice"];

/// The danger levels, mildest first.
pub const DANGER_LEVELS: &[&str] = &["read", "safe", "caution", "destructive"];

/// The longest a text input may be when its entry gives no `max_len`.
const DEFAULT_MAX_LEN: usize = 1_000;

impl ActionDef {
    /// Runs through `POST /api/admin/actions/run`.
    pub fn runs_here(&self) -> bool {
        self.run.as_deref() == Some("here")
    }

    /// Changes nothing (not written to the admin log).
    pub fn is_read(&self) -> bool {
        self.danger == "read"
    }

    /// Cannot be undone: needs its confirm word.
    pub fn is_destructive(&self) -> bool {
        self.danger == "destructive"
    }

    /// The chat command's first word, e.g. `/server-add`.
    pub fn chat_command(&self) -> Option<&str> {
        self.chat.as_deref().and_then(|c| c.split_whitespace().next())
    }

    /// The chat command with the inputs filled in; an empty optional input leaves no gap.
    pub fn chat_text(&self, inputs: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
        let template = self.chat.as_deref()?;
        let words: Vec<String> = template
            .split_whitespace()
            .map(|w| {
                if let Some(name) = w.strip_prefix('{').and_then(|r| r.strip_suffix('}')) {
                    match inputs.get(name) {
                        Some(serde_json::Value::String(s)) => s.trim().to_string(),
                        Some(serde_json::Value::Number(n)) => n.to_string(),
                        Some(serde_json::Value::Bool(b)) => b.to_string(),
                        _ => String::new(),
                    }
                } else {
                    w.to_string()
                }
            })
            .filter(|w| !w.is_empty())
            .collect();
        Some(words.join(" "))
    }
}

/// The registry built into this relay. The file is checked by `tests.rs`, so it always parses.
pub fn registry() -> &'static Registry {
    static R: OnceLock<Registry> = OnceLock::new();
    R.get_or_init(|| serde_json::from_str(REGISTRY_JSON).expect("data/admin/actions.json parses (src/relay/admin/tests.rs)"))
}

/// The entry for `id`.
pub fn find(id: &str) -> Option<&'static ActionDef> {
    registry().actions.iter().find(|a| a.id == id)
}

/// What a run signs (before `\n<timestamp>`): the action and the BLAKE3 of the exact inputs text.
/// Both apps build the same words: web/shared/admin-actions.js `adminRunPurpose`, and the desktop
/// app calls this function itself.
pub fn run_purpose(action: &str, inputs_text: &str) -> String {
    format!("{RUN_PURPOSE_PREFIX}\n{action}\n{}", blake3::hash(inputs_text.as_bytes()).to_hex())
}

/// How much a role may do: 3 owner or admin, 2 moderator, 1 anyone else.
pub fn role_rank(role: &str) -> u8 {
    match role {
        "owner" | "admin" => 3,
        "mod" | "moderator" => 2,
        _ => 1,
    }
}

/// What an entry's `role` asks of the caller's [`role_rank`].
pub fn needed_rank(role: &str) -> u8 {
    match role {
        "admin" => 3,
        "mod" => 2,
        _ => 1,
    }
}

/// Who is running an action: their key and their role here, read after their signature checked.
#[derive(Debug, Clone)]
pub struct Caller {
    pub key: String,
    pub role: String,
}

/// An action's inputs once checked against its entry.
#[derive(Debug, Clone, Default)]
pub struct Inputs(pub serde_json::Map<String, serde_json::Value>);

impl Inputs {
    /// A text input, trimmed; None when absent or empty.
    pub fn text(&self, name: &str) -> Option<String> {
        self.0.get(name).and_then(|v| v.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
    }

    /// A number input.
    pub fn number(&self, name: &str) -> Option<i64> {
        self.0.get(name).and_then(|v| v.as_i64())
    }

    /// A yes-or-no input.
    pub fn flag(&self, name: &str) -> Option<bool> {
        self.0.get(name).and_then(|v| v.as_bool())
    }
}

/// What an action did.
#[derive(Debug, Clone)]
pub struct Ran {
    /// One sentence for the person (or AI) who ran it.
    pub message: String,
    /// What a panel draws from (a list, the new state).
    pub data: serde_json::Value,
    /// The line for the admin log, when it should differ from `message` (it never holds a
    /// token, an address or a person's key).
    pub log: Option<String>,
}

impl Ran {
    pub fn said(message: impl Into<String>) -> Self {
        Ran { message: message.into(), data: serde_json::Value::Null, log: None }
    }

    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = data;
        self
    }

    pub fn logged_as(mut self, log: impl Into<String>) -> Self {
        self.log = Some(log.into());
        self
    }
}

/// Why an action did not run, with the HTTP status that says so.
#[derive(Debug, Clone)]
pub struct Refusal {
    pub status: StatusCode,
    pub message: String,
    /// The admin log's line, when the message must not be kept (it names a person).
    pub log: Option<String>,
}

impl Refusal {
    pub fn with(status: StatusCode, message: impl Into<String>) -> Self {
        Refusal { status, message: message.into(), log: None }
    }

    pub fn bad(message: impl Into<String>) -> Self {
        Self::with(StatusCode::BAD_REQUEST, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::with(StatusCode::NOT_FOUND, message)
    }

    pub fn failed(message: impl Into<String>) -> Self {
        Self::with(StatusCode::INTERNAL_SERVER_ERROR, message)
    }

    pub fn logged_as(mut self, log: impl Into<String>) -> Self {
        self.log = Some(log.into());
        self
    }

    fn response(&self) -> Response {
        (self.status, Json(serde_json::json!({ "ok": false, "error": self.message }))).into_response()
    }
}

/// The future a runner returns.
pub type RunFuture = Pin<Box<dyn Future<Output = Result<Ran, Refusal>> + Send>>;

/// A runner: the state, who is running it and the checked inputs.
pub type Runner = fn(Arc<RelayState>, Caller, Inputs) -> RunFuture;

fn boxed<F: Future<Output = Result<Ran, Refusal>> + Send + 'static>(f: F) -> RunFuture {
    Box::pin(f)
}

/// Every action that runs here, and what runs it. `tests.rs` checks this against the registry in
/// both directions. A `backups.` action moved off `POST /api/admin/backups` gets its line here.
pub const RUNNERS: &[(&str, Runner)] = &[
    ("alerts.list", |s, c, i| boxed(alerts::list(s, c, i))),
    ("alerts.add", |s, c, i| boxed(alerts::add(s, c, i))),
    ("alerts.update", |s, c, i| boxed(alerts::update(s, c, i))),
    ("alerts.set_enabled", |s, c, i| boxed(alerts::set_enabled(s, c, i))),
    ("alerts.remove", |s, c, i| boxed(alerts::remove(s, c, i))),
    ("alerts.test", |s, c, i| boxed(alerts::test(s, c, i))),
    ("federation.list", |s, c, i| boxed(federation::list(s, c, i))),
    ("federation.add", |s, c, i| boxed(federation::add(s, c, i))),
    ("federation.add_key", |s, c, i| boxed(federation::add_key(s, c, i))),
    ("federation.trust", |s, c, i| boxed(federation::trust(s, c, i))),
    ("federation.pause", |s, c, i| boxed(federation::pause(s, c, i))),
    ("federation.resume", |s, c, i| boxed(federation::resume(s, c, i))),
    ("federation.recheck", |s, c, i| boxed(federation::recheck(s, c, i))),
    ("federation.remove", |s, c, i| boxed(federation::remove(s, c, i))),
    ("federation.connect", |s, c, i| boxed(federation::connect(s, c, i))),
    ("federation.channel", |s, c, i| boxed(federation::channel(s, c, i))),
    ("log.recent", |s, c, i| boxed(log_recent(s, c, i))),
    ("members.kick", |s, c, i| boxed(members::moderate(s, c, i, "/kick"))),
    ("members.mute", |s, c, i| boxed(members::moderate(s, c, i, "/mute"))),
    ("members.unmute", |s, c, i| boxed(members::moderate(s, c, i, "/unmute"))),
    ("members.ban", |s, c, i| boxed(members::moderate(s, c, i, "/ban"))),
    ("members.unban", |s, c, i| boxed(members::moderate(s, c, i, "/unban"))),
    ("members.mod", |s, c, i| boxed(members::moderate(s, c, i, "/mod"))),
    ("members.unmod", |s, c, i| boxed(members::moderate(s, c, i, "/unmod"))),
];

/// The runner for `id`.
pub fn runner_for(id: &str) -> Option<Runner> {
    RUNNERS.iter().find(|(name, _)| *name == id).map(|(_, r)| *r)
}

/// Check `raw` (a JSON object) against `def`'s inputs: no input it does not take, every required
/// one present and non-empty, each of the right kind and within its limits.
pub fn check_inputs(def: &ActionDef, raw: &serde_json::Value) -> Result<Inputs, String> {
    let empty = serde_json::Map::new();
    let map = match raw {
        serde_json::Value::Object(m) => m,
        serde_json::Value::Null => &empty,
        _ => return Err("The inputs must be a JSON object.".into()),
    };
    for name in map.keys() {
        if !def.inputs.iter().any(|i| &i.name == name) {
            return Err(format!("{} takes no input named \"{name}\".", def.id));
        }
    }
    let mut out = serde_json::Map::new();
    for input in &def.inputs {
        let value = match map.get(&input.name) {
            None | Some(serde_json::Value::Null) => None,
            Some(v) => Some(v),
        };
        let Some(value) = value else {
            if input.required {
                return Err(format!("{} is needed.", input.label));
            }
            continue;
        };
        let checked = match input.kind.as_str() {
            "text" | "secret" => {
                let s = value.as_str().ok_or_else(|| format!("{} must be text.", input.label))?.trim();
                if s.chars().count() > input.max_len.unwrap_or(DEFAULT_MAX_LEN) {
                    return Err(format!("{} is too long (at most {} characters).", input.label, input.max_len.unwrap_or(DEFAULT_MAX_LEN)));
                }
                if s.chars().any(|c| c.is_control()) {
                    return Err(format!("{} cannot hold line breaks or control characters.", input.label));
                }
                if s.is_empty() {
                    if input.required {
                        return Err(format!("{} is needed.", input.label));
                    }
                    continue;
                }
                serde_json::Value::String(s.to_string())
            }
            "number" => {
                let n = value.as_i64().ok_or_else(|| format!("{} must be a whole number.", input.label))?;
                if input.min.is_some_and(|m| n < m) || input.max.is_some_and(|m| n > m) {
                    return Err(format!(
                        "{} must be from {} to {}.",
                        input.label,
                        input.min.map(|m| m.to_string()).unwrap_or_else(|| "any".into()),
                        input.max.map(|m| m.to_string()).unwrap_or_else(|| "any".into())
                    ));
                }
                serde_json::Value::from(n)
            }
            "bool" => serde_json::Value::Bool(value.as_bool().ok_or_else(|| format!("{} must be true or false.", input.label))?),
            "choice" => {
                let s = match value {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    _ => return Err(format!("{} must be one of: {}.", input.label, input.choices.join(", "))),
                };
                if !input.choices.iter().any(|c| c == &s) {
                    return Err(format!("{} must be one of: {}.", input.label, input.choices.join(", ")));
                }
                serde_json::Value::String(s)
            }
            other => return Err(format!("{} has an input kind this relay does not know ({other}).", def.id)),
        };
        out.insert(input.name.clone(), checked);
    }
    Ok(Inputs(out))
}

/// The registry as served: the file, each action with `destructive` spelled out, and which
/// server and build is answering.
pub fn served_registry() -> serde_json::Value {
    let mut v: serde_json::Value = serde_json::from_str(REGISTRY_JSON).unwrap_or_default();
    if let Some(actions) = v.get_mut("actions").and_then(|a| a.as_array_mut()) {
        for a in actions.iter_mut() {
            let destructive = a.get("danger").and_then(|d| d.as_str()) == Some("destructive");
            if let Some(obj) = a.as_object_mut() {
                obj.insert("destructive".into(), serde_json::Value::Bool(destructive));
            }
        }
    }
    if let Some(obj) = v.as_object_mut() {
        obj.remove("_purpose");
        obj.insert(
            "server".into(),
            serde_json::json!({
                "name": std::env::var("SERVER_NAME").unwrap_or_else(|_| "Humanity Relay".to_string()),
                "version": env!("CARGO_PKG_VERSION"),
            }),
        );
        obj.insert(
            "endpoints".into(),
            serde_json::json!({ "list": "GET /api/admin/actions", "run": "POST /api/admin/actions/run" }),
        );
    }
    v
}

/// GET /api/admin/actions: the registry, to anyone (see the module comment for why).
pub async fn get_actions() -> Json<serde_json::Value> {
    Json(served_registry())
}

/// Body of POST /api/admin/actions/run.
#[derive(Debug, Deserialize)]
pub struct RunRequest {
    /// The caller's Dilithium3 public key (hex).
    #[serde(default)]
    pub key: String,
    /// Milliseconds since 1970 when signed: fresh (5 minutes) and used once.
    #[serde(default)]
    pub timestamp: u64,
    /// Dilithium3 signature (hex) over `run_purpose(action, inputs)` + `\n` + timestamp.
    #[serde(default)]
    pub sig: String,
    /// The action's id.
    #[serde(default)]
    pub action: String,
    /// The inputs as JSON TEXT: exactly the bytes the signature's hash covers. Empty means `{}`.
    #[serde(default)]
    pub inputs: String,
    /// A destructive action's confirm word.
    #[serde(default)]
    pub confirm: Option<String>,
}

/// Check a signed admin request: present, fresh (5 minutes, not from the future), the signature
/// over `purpose`, and not used before. Returns the caller with their role here. The role itself
/// is checked by the caller against what its action needs.
pub async fn check_signed(state: &RelayState, key: &str, timestamp: u64, sig: &str, tag: &str, purpose: String) -> Result<Caller, Refusal> {
    use crate::relay::handlers::broadcast::verify_dilithium_signature;
    if key.is_empty() || sig.is_empty() {
        return Err(Refusal::with(StatusCode::UNAUTHORIZED, "Sign the request with your key: admin actions are never run unsigned."));
    }
    let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    if now_ms.saturating_sub(timestamp) > 5 * 60 * 1000 || timestamp > now_ms + 60_000 {
        return Err(Refusal::bad("The request is too old or from the future: check this computer's clock and try again."));
    }
    let (vk, vsig) = (key.to_string(), sig.to_string());
    // CPU-bound: off the async executor, failing closed if the check panics.
    let ok = tokio::task::spawn_blocking(move || verify_dilithium_signature(&vk, &purpose, timestamp, &vsig)).await.unwrap_or(false);
    if !ok {
        return Err(Refusal::with(StatusCode::UNAUTHORIZED, "The signature does not match this request."));
    }
    if !state.auth_nonce_fresh(key, tag, timestamp) {
        return Err(Refusal::with(StatusCode::UNAUTHORIZED, "That request was already used: try again."));
    }
    Ok(Caller { key: key.to_string(), role: state.db.get_role(key).unwrap_or_default() })
}

/// What a role is called in a refusal.
fn role_words(role: &str) -> &'static str {
    match role {
        "admin" => "this server's admins",
        "mod" => "this server's moderators and admins",
        _ => "people signed in here",
    }
}

/// POST /api/admin/actions/run: run one action (see the module comment for the order of checks).
pub async fn post_run(State(state): State<Arc<RelayState>>, Json(req): Json<RunRequest>) -> Response {
    let Some(def) = find(&req.action) else {
        return Refusal::not_found(format!("There is no action named \"{}\". GET /api/admin/actions lists them.", req.action)).response();
    };
    if !def.runs_here() {
        let how = match (&def.chat, &def.route) {
            (Some(chat), _) => format!("it runs as the chat command {chat}"),
            (None, Some(route)) => format!("it runs at {route}"),
            _ => "it has no way to run yet".to_string(),
        };
        return Refusal::bad(format!("{} does not run here: {how}.", def.id)).response();
    }
    let inputs_text = if req.inputs.trim().is_empty() { "{}".to_string() } else { req.inputs.clone() };
    let caller = match check_signed(&state, &req.key, req.timestamp, &req.sig, RUN_NONCE_TAG, run_purpose(&def.id, &req.inputs)).await {
        Ok(c) => c,
        Err(r) => return r.response(),
    };
    if let Some(refused) = role_refusal(def, &caller.role) {
        return refused.response();
    }
    if let Some(refused) = confirm_refusal(def, req.confirm.as_deref()) {
        return refused.response();
    }
    let raw: serde_json::Value = match serde_json::from_str(&inputs_text) {
        Ok(v) => v,
        Err(_) => return Refusal::bad("The inputs are not valid JSON.").response(),
    };
    let inputs = match check_inputs(def, &raw) {
        Ok(i) => i,
        Err(why) => return Refusal::bad(why).response(),
    };
    let Some(runner) = runner_for(&def.id) else {
        return Refusal::failed(format!("{} is registered to run here but this relay has no runner for it.", def.id)).response();
    };
    let result = runner(state.clone(), caller.clone(), inputs).await;
    if !def.is_read() {
        let (summary, ok) = match &result {
            Ok(ran) => (ran.log.clone().unwrap_or_else(|| ran.message.clone()), true),
            Err(r) => (r.log.clone().unwrap_or_else(|| format!("Refused: {}", r.message)), false),
        };
        if let Err(e) = state.db.admin_log_append(&caller.key, &def.id, &summary, ok) {
            tracing::error!("admin log: could not record {}: {e}", def.id);
        }
        // The relay's own log names the action and whether it worked: never the inputs, never
        // the key (the admin log keeps who, as a fingerprint).
        tracing::info!("Admin action {} ({})", def.id, if ok { "done" } else { "refused" });
    }
    match result {
        Ok(ran) => (StatusCode::OK, Json(serde_json::json!({ "ok": true, "action": def.id, "message": ran.message, "data": ran.data }))).into_response(),
        Err(r) => r.response(),
    }
}

/// Why `role` may not run `def`, when it may not.
pub fn role_refusal(def: &ActionDef, role: &str) -> Option<Refusal> {
    (role_rank(role) < needed_rank(&def.role))
        .then(|| Refusal::with(StatusCode::FORBIDDEN, format!("Only {} can {}.", role_words(&def.role), lower_first(&def.label))))
}

/// Why a destructive `def` may not run with `confirm`, when it may not: its confirm word must be
/// sent, exactly (trimmed). Never asked of an action that can be undone.
pub fn confirm_refusal(def: &ActionDef, confirm: Option<&str>) -> Option<Refusal> {
    if !def.is_destructive() {
        return None;
    }
    let word = def.confirm_word.as_deref().unwrap_or("CONFIRM");
    (confirm.map(str::trim) != Some(word))
        .then(|| Refusal::bad(format!("{} cannot be undone: send confirm \"{word}\" to run it.", def.label)))
}

/// `Remove a peer` as it reads after "Only admins can ...".
fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// `log.recent`: the newest 100 rows of the admin log, each with the label of its action and the
/// name of the admin who ran it while they still hold a role here.
async fn log_recent(state: Arc<RelayState>, _caller: Caller, _inputs: Inputs) -> Result<Ran, Refusal> {
    let rows = state.db.admin_log_recent(100).map_err(|e| Refusal::failed(format!("The admin log could not be read: {e}")))?;
    let names = state.db.admin_log_names();
    let entries: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "at": r.at_ms,
                "action": r.action,
                "label": find(&r.action).map(|a| a.label.clone()).unwrap_or_else(|| r.action.clone()),
                "who": names.get(&r.admin_fingerprint).cloned().unwrap_or_else(|| "a former admin".to_string()),
                "summary": r.summary,
                "ok": r.ok,
            })
        })
        .collect();
    let n = entries.len();
    Ok(Ran::said(if n == 0 { "Nothing has been changed through the admin actions yet.".to_string() } else { format!("The newest {n} changes.") })
        .with_data(serde_json::json!({ "entries": entries })))
}
