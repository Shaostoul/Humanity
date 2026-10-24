//! Alert channels, the first panel that WRITES (2026-10-22; docs/design/in-app-ops.md, "Alert
//! channels", the debt the doc called the freshest).
//!
//! The relay watchdog and the disk guard fan a warning out to every enabled channel in
//! `data/alert-channels.secrets.json` through `scripts/humanity-alert.js`: ntfy (a phone
//! notification), Discord, Slack or any web address (a webhook), Telegram. Until now an admin
//! edited that file over SSH. These are the actions that edit it from the app: list, add,
//! change, turn on or off, remove, and send a test.
//!
//! THE FILE stays the one the script reads, in the shape it reads: `{"channels": [{"type",
//! "label", "enabled", ...}]}` with the address under the field the script expects for its kind
//! ([`address_field`]). `HUMANITY_ALERT_CONFIG` moves it for both, as it always moved it for the
//! script. Fields this panel does not edit (a ntfy `title`, a webhook `method`) are kept as they
//! are. Written to a temporary file and renamed into place, readable by its owner only on Unix,
//! so a crash never leaves half a file and nobody else on the machine reads the tokens.
//!
//! SECRETS NEVER LEAVE. An address or token is shown back only as its host and its last few
//! characters ([`masked`]); the admin log records the channel's kind and label, never its address;
//! and a failed send reports what the other end answered without the address (reqwest's error
//! text carries the URL, so it is stripped with `without_url`).
//!
//! EMAIL. There is no email channel: the relay holds no mail server settings, and asking for them
//! here would make this panel the keeper of a mail password. An email service's webhook address
//! works as a Webhook channel.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use super::{Caller, Inputs, Ran, Refusal};
use crate::relay::relay::RelayState;

/// The kinds of channel, in the order the apps offer them.
pub const KINDS: &[&str] = &["ntfy", "discord", "webhook", "telegram"];

/// The most channels the file may hold.
pub const MAX_CHANNELS: usize = 50;

/// How long a test waits for the other end.
const SEND_TIMEOUT: Duration = Duration::from_secs(10);

/// The note written at the top of the file.
const README: &str = "Alert channels for this HumanityOS server, edited in the app (Server Settings > ADMIN > Alert channels, or the website's admin page). The relay watchdog and disk guard send warnings to every enabled channel through scripts/humanity-alert.js. Holds tokens and addresses: keep it private.";

/// One read-modify-write at a time. Held only around file work, never across an await.
static FILE_LOCK: Mutex<()> = Mutex::new(());

/// Where the file is: `HUMANITY_ALERT_CONFIG` when set (the script reads the same), otherwise
/// `data/alert-channels.secrets.json` beside the relay's database folder.
pub fn config_path() -> PathBuf {
    std::env::var("HUMANITY_ALERT_CONFIG")
        .ok()
        .filter(|p| !p.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data").join("alert-channels.secrets.json"))
}

/// The field a kind keeps its address or token in, as scripts/humanity-alert.js reads it.
pub fn address_field(kind: &str) -> &'static str {
    match kind {
        "ntfy" => "topic",
        "discord" => "webhook_url",
        "telegram" => "bot_token",
        _ => "url",
    }
}

/// A channel's kind. The script reads `slack` as a webhook, so it is shown as one.
pub fn kind_of(ch: &Value) -> String {
    match ch.get("type").and_then(|t| t.as_str()).unwrap_or("") {
        "slack" => "webhook".to_string(),
        other => other.to_string(),
    }
}

/// A channel's address or token, wherever its kind keeps it (a webhook may use either name).
fn address_of(ch: &Value) -> String {
    let kind = kind_of(ch);
    let primary = ch.get(address_field(&kind)).and_then(|v| v.as_str()).unwrap_or("");
    if primary.is_empty() && kind == "webhook" {
        return ch.get("webhook_url").and_then(|v| v.as_str()).unwrap_or("").to_string();
    }
    primary.to_string()
}

/// On unless it says `"enabled": false`, as the script reads it.
pub fn is_enabled(ch: &Value) -> bool {
    ch.get("enabled").and_then(|v| v.as_bool()) != Some(false)
}

fn label_of(ch: &Value) -> String {
    let label = ch.get("label").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if label.is_empty() { kind_of(ch) } else { label }
}

/// How a channel is named in a message and in the admin log: its kind and label, never its
/// address.
pub fn named(ch: &Value) -> String {
    format!("{} channel \"{}\"", kind_of(ch), label_of(ch))
}

/// A secret as it may be shown: whether it is set, the web address's host when it is one, and
/// its last few characters.
pub fn mask(secret: &str) -> Value {
    let secret = secret.trim();
    if secret.is_empty() {
        return json!({ "set": false });
    }
    let host = reqwest::Url::parse(secret).ok().and_then(|u| u.host_str().map(str::to_string));
    let n = secret.chars().count();
    let shown = if n >= 16 { 4 } else if n >= 8 { 2 } else { 0 };
    let ends: String = secret.chars().skip(n - shown).collect();
    json!({ "set": true, "host": host, "ends": ends })
}

/// One channel as the apps are shown it: nothing whole but the kind, the label and on or off.
pub fn masked(index: usize, ch: &Value) -> Value {
    json!({
        "index": index,
        "kind": kind_of(ch),
        "label": label_of(ch),
        "enabled": is_enabled(ch),
        "address": mask(&address_of(ch)),
        "chat_id": mask(ch.get("chat_id").and_then(|v| v.as_str()).unwrap_or("")),
    })
}

/// The file's channels. A missing file is no channels: alerting is opt-in.
pub fn read_channels(path: &Path) -> Result<Vec<Value>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("The alert file could not be read ({}).", e.kind())),
    };
    let parsed: Value = serde_json::from_str(&text)
        .map_err(|_| format!("The alert file at {} is not valid JSON. Fix it or remove it on the server, then try again.", path.display()))?;
    let list = match &parsed {
        Value::Array(a) => a.clone(),
        Value::Object(o) => o.get("channels").and_then(|c| c.as_array()).cloned().unwrap_or_default(),
        _ => Vec::new(),
    };
    Ok(list.into_iter().filter(|c| c.is_object()).collect())
}

/// Write the channels back: a temporary file renamed into place, owner-only on Unix.
pub fn write_channels(path: &Path, channels: &[Value]) -> Result<(), String> {
    let doc = json!({ "_README": README, "channels": channels });
    let text = serde_json::to_string_pretty(&doc).map_err(|e| format!("The channels could not be written: {e}"))?;
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("The alert file's folder could not be made ({}).", e.kind()))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text.as_bytes()).map_err(|e| format!("The alert file could not be written ({}).", e.kind()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("The alert file could not be replaced ({}).", e.kind())
    })
}

/// Whether `address` is a usable address or token for `kind`. The reason, when not.
pub fn check_address(kind: &str, address: &str) -> Result<(), String> {
    let address = address.trim();
    if kind == "telegram" {
        let ok = address.split_once(':').is_some_and(|(bot, rest)| {
            !bot.is_empty() && bot.chars().all(|c| c.is_ascii_digit()) && rest.len() >= 20 && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        });
        return if ok { Ok(()) } else { Err("A Telegram bot token looks like 123456:ABC-DEF... (the numbers, a colon, then at least 20 letters and numbers), from @BotFather.".into()) };
    }
    let url = reqwest::Url::parse(address).map_err(|_| "That is not a web address: it should start with https://".to_string())?;
    if url.host_str().map_or(true, |h| h.is_empty()) {
        return Err("That web address has no server name in it.".into());
    }
    match (kind, url.scheme()) {
        ("discord", "https") => Ok(()),
        ("discord", _) => Err("A Discord webhook address starts with https://".into()),
        (_, "https" | "http") => {
            if kind == "ntfy" && url.path().trim_matches('/').is_empty() {
                Err("An ntfy address ends with the topic name: https://ntfy.sh/a-long-random-name".into())
            } else {
                Ok(())
            }
        }
        _ => Err("The address must start with https:// (or http:// for a server on your own network).".into()),
    }
}

/// Whether `id` is a Telegram chat: a number (negative for a group) or an @channel name.
pub fn check_chat_id(id: &str) -> Result<(), String> {
    let id = id.trim();
    let number = id.strip_prefix('-').unwrap_or(id);
    let ok = (!number.is_empty() && number.len() <= 20 && number.chars().all(|c| c.is_ascii_digit()))
        || id.strip_prefix('@').is_some_and(|n| (4..=64).contains(&n.len()) && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
    if ok { Ok(()) } else { Err("A Telegram chat is its number (negative for a group) or an @channel name.".into()) }
}

/// Every channel as the apps see it.
fn view(channels: &[Value]) -> Value {
    json!({
        "channels": channels.iter().enumerate().map(|(i, c)| masked(i, c)).collect::<Vec<_>>(),
        "file": config_path().display().to_string(),
        "kinds": KINDS,
    })
}

/// Read, change and write the file under the lock. `change` returns the message and the log
/// line, or why it refused.
fn edit(change: impl FnOnce(&mut Vec<Value>) -> Result<(String, String), Refusal>) -> Result<Ran, Refusal> {
    let _guard = FILE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let path = config_path();
    let mut channels = read_channels(&path).map_err(Refusal::failed)?;
    let (message, log) = change(&mut channels)?;
    write_channels(&path, &channels).map_err(Refusal::failed)?;
    Ok(Ran::said(message).logged_as(log).with_data(view(&channels)))
}

fn index_of(inputs: &Inputs, channels: &[Value]) -> Result<usize, Refusal> {
    let i = inputs.number("index").unwrap_or(-1);
    if i < 0 || i as usize >= channels.len() {
        return Err(Refusal::not_found(format!("There is no alert channel number {i}: the list has {}.", channels.len())));
    }
    Ok(i as usize)
}

/// `alerts.list`
pub async fn list(_state: Arc<RelayState>, _caller: Caller, _inputs: Inputs) -> Result<Ran, Refusal> {
    let channels = {
        let _guard = FILE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        read_channels(&config_path()).map_err(Refusal::failed)?
    };
    let on = channels.iter().filter(|c| is_enabled(c)).count();
    let message = match channels.len() {
        0 => "No alert channels yet: this server's warnings go nowhere until you add one.".to_string(),
        n => format!("{n} alert channel{}, {on} on.", if n == 1 { "" } else { "s" }),
    };
    Ok(Ran::said(message).with_data(view(&channels)))
}

/// `alerts.add`
pub async fn add(_state: Arc<RelayState>, _caller: Caller, inputs: Inputs) -> Result<Ran, Refusal> {
    let kind = inputs.text("kind").unwrap_or_default();
    if !KINDS.contains(&kind.as_str()) {
        return Err(Refusal::bad(format!("The kind must be one of: {}.", KINDS.join(", "))));
    }
    let address = inputs.text("address").unwrap_or_default();
    check_address(&kind, &address).map_err(Refusal::bad)?;
    let chat_id = inputs.text("chat_id");
    if kind == "telegram" {
        check_chat_id(chat_id.as_deref().unwrap_or("")).map_err(Refusal::bad)?;
    }
    let label = inputs.text("label").unwrap_or_else(|| kind.clone());
    let enabled = inputs.flag("enabled").unwrap_or(true);
    edit(|channels| {
        if channels.len() >= MAX_CHANNELS {
            return Err(Refusal::bad(format!("This server already has {MAX_CHANNELS} alert channels, the most it keeps.")));
        }
        let mut ch = json!({ "type": kind, "label": label, "enabled": enabled });
        ch[address_field(&kind)] = Value::String(address.clone());
        if kind == "telegram" {
            ch["chat_id"] = Value::String(chat_id.clone().unwrap_or_default());
        }
        if kind == "ntfy" {
            ch["title"] = Value::String(std::env::var("SERVER_NAME").map(|n| format!("HumanityOS: {n}")).unwrap_or_else(|_| "HumanityOS Relay".into()));
        }
        let name = named(&ch);
        channels.push(ch);
        Ok((
            format!("Added the {name}{}. Send it a test to see one arrive.", if enabled { "" } else { ", off for now" }),
            format!("Added the {name}{}.", if enabled { "" } else { ", off" }),
        ))
    })
}

/// `alerts.update`
pub async fn update(_state: Arc<RelayState>, _caller: Caller, inputs: Inputs) -> Result<Ran, Refusal> {
    let label = inputs.text("label");
    let address = inputs.text("address");
    let chat_id = inputs.text("chat_id");
    if label.is_none() && address.is_none() && chat_id.is_none() {
        return Err(Refusal::bad("Nothing to change: give a new name, address or chat number."));
    }
    edit(|channels| {
        let i = index_of(&inputs, channels)?;
        let kind = kind_of(&channels[i]);
        if let Some(a) = &address {
            check_address(&kind, a).map_err(Refusal::bad)?;
        }
        if let Some(c) = &chat_id {
            if kind != "telegram" {
                return Err(Refusal::bad("Only a Telegram channel has a chat number."));
            }
            check_chat_id(c).map_err(Refusal::bad)?;
        }
        let before = named(&channels[i]);
        let ch = &mut channels[i];
        let mut changed = Vec::new();
        if let Some(l) = &label {
            ch["label"] = Value::String(l.clone());
            changed.push("its name");
        }
        if let Some(a) = &address {
            // A webhook kept under the other field name moves to the one this panel writes.
            if kind == "webhook" {
                if let Some(o) = ch.as_object_mut() {
                    o.remove("webhook_url");
                }
            }
            ch[address_field(&kind)] = Value::String(a.clone());
            changed.push(if kind == "telegram" { "its token" } else { "its address" });
        }
        if let Some(c) = &chat_id {
            ch["chat_id"] = Value::String(c.clone());
            changed.push("its chat number");
        }
        let what = changed.join(" and ");
        Ok((format!("Changed {what} for the {before}."), format!("Changed {what} for the {before}.")))
    })
}

/// `alerts.set_enabled`
pub async fn set_enabled(_state: Arc<RelayState>, _caller: Caller, inputs: Inputs) -> Result<Ran, Refusal> {
    let on = inputs.flag("enabled").unwrap_or(true);
    edit(|channels| {
        let i = index_of(&inputs, channels)?;
        channels[i]["enabled"] = Value::Bool(on);
        let name = named(&channels[i]);
        let words = if on { format!("Turned on the {name}.") } else { format!("Turned off the {name}: it is kept, and gets nothing until it is on again.") };
        Ok((words, format!("Turned {} the {name}.", if on { "on" } else { "off" })))
    })
}

/// `alerts.remove`
pub async fn remove(_state: Arc<RelayState>, _caller: Caller, inputs: Inputs) -> Result<Ran, Refusal> {
    edit(|channels| {
        let i = index_of(&inputs, channels)?;
        let gone = channels.remove(i);
        let name = named(&gone);
        Ok((format!("Removed the {name}."), format!("Removed the {name}.")))
    })
}

/// `alerts.test`: one real message to one channel, off or on.
pub async fn test(state: Arc<RelayState>, _caller: Caller, inputs: Inputs) -> Result<Ran, Refusal> {
    let ch = {
        let _guard = FILE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let channels = read_channels(&config_path()).map_err(Refusal::failed)?;
        let i = index_of(&inputs, &channels)?;
        channels[i].clone()
    };
    let name = named(&ch);
    let server = std::env::var("SERVER_NAME").unwrap_or_else(|_| "this HumanityOS server".into());
    let text = format!("Test alert from {server}: an admin sent this from the Alert channels panel. If you can read it, this server's warnings reach you here.");
    match deliver(&state.http_client, &ch, &text, "info").await {
        Ok(status) if (200..300).contains(&status) => Ok(Ran::said(format!("Sent a test to the {name}: it answered {status}, so it arrived. Check that you see it."))
            .logged_as(format!("Sent a test to the {name}: delivered ({status})."))),
        Ok(status) => Err(Refusal::with(axum::http::StatusCode::BAD_GATEWAY, format!("The {name} answered {status}: {}", answered_words(status)))),
        Err(why) => Err(Refusal::with(axum::http::StatusCode::BAD_GATEWAY, format!("The test to the {name} did not arrive: {why}"))),
    }
}

/// What an answer from the other end means, in words.
pub fn answered_words(status: u16) -> &'static str {
    match status {
        400 => "it did not accept the message (for Telegram, check the chat number).",
        401 | 403 => "it refused: the token or address is wrong, or no longer allowed.",
        404 => "nothing there: the address is wrong, or the channel or webhook was deleted.",
        429 => "too many messages just now: try again in a minute.",
        500..=599 => "the other service had a problem: try again later.",
        _ => "it did not take the message.",
    }
}

/// Send `text` to one channel, the way scripts/humanity-alert.js does for its kind. The answer's
/// status, or why nothing came back (never with the address in it).
pub async fn deliver(client: &reqwest::Client, ch: &Value, text: &str, severity: &str) -> Result<u16, String> {
    let kind = kind_of(ch);
    let address = address_of(ch);
    if address.is_empty() {
        return Err("it has no address or token saved.".into());
    }
    let request = match kind.as_str() {
        "ntfy" => {
            let mut r = client.post(&address).header("Content-Type", "text/plain").body(text.to_string());
            if let Some(title) = ch.get("title").and_then(|t| t.as_str()) {
                r = r.header("Title", title);
            }
            match severity {
                "critical" => r.header("Priority", "urgent").header("Tags", "rotating_light"),
                "warn" => r.header("Priority", "high"),
                _ => r,
            }
        }
        "discord" => client.post(&address).json(&json!({ "content": text.chars().take(1900).collect::<String>() })),
        "webhook" => {
            let method = ch.get("method").and_then(|m| m.as_str()).unwrap_or("POST").to_ascii_uppercase();
            let method = reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::POST);
            client.request(method, &address).json(&json!({ "text": text, "message": text, "content": text, "severity": severity }))
        }
        "telegram" => {
            let chat = ch.get("chat_id").and_then(|c| c.as_str()).unwrap_or("");
            client.post(format!("https://api.telegram.org/bot{address}/sendMessage")).json(&json!({ "chat_id": chat, "text": text }))
        }
        other => return Err(format!("this relay does not know the kind \"{other}\".")),
    };
    match request.timeout(SEND_TIMEOUT).send().await {
        Ok(resp) => Ok(resp.status().as_u16()),
        Err(e) => {
            let e = e.without_url();
            Err(if e.is_timeout() {
                "no answer within 10 seconds.".to_string()
            } else if e.is_connect() {
                "this server could not connect to it (is the address right, and is that service up?).".to_string()
            } else {
                format!("the request failed ({e}).")
            })
        }
    }
}
