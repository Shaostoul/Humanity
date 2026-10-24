//! The desktop app's half of the server's action registry (2026-10-22; src/relay/admin/,
//! docs/design/in-app-ops.md): fetch the list (`GET /api/admin/actions`), run an action
//! (`POST /api/admin/actions/run`, signed with this identity's Dilithium3 key over
//! `relay::admin::run_purpose`, the very function the relay checks with), and take the answers in
//! each frame. Every request runs on a worker thread, so the page never waits on the network; the
//! answers come back over `AdminConsoleState::tx`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::{json, Value};

use crate::gui::{AdminReply, GuiState};

/// The time a request is signed at: the next millisecond after the last one this app signed, so
/// two requests started together are never refused as the same one (the relay remembers each
/// key, purpose and time once).
fn next_timestamp() -> u64 {
    static LAST: AtomicU64 = AtomicU64::new(0);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    let mut prev = LAST.load(Ordering::SeqCst);
    loop {
        let next = if now > prev { now } else { prev + 1 };
        match LAST.compare_exchange(prev, next, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => return next,
            Err(p) => prev = p,
        }
    }
}

/// The connected server's web address, or None while the app is not connected to one.
pub(crate) fn server_base(state: &GuiState) -> Option<String> {
    let connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());
    (connected && !state.server_url.is_empty()).then(|| state.server_url.trim_end_matches('/').to_string())
}

/// The views belong to one server: talking to another drops them.
fn track_server(state: &mut GuiState, base: &str) {
    if state.admin_console.server != base {
        state.admin_console.forget_views();
        state.admin_console.server = base.to_string();
    }
}

/// Fetch the server's registry (`GET /api/admin/actions`).
pub(crate) fn fetch_registry(state: &mut GuiState) {
    let Some(base) = server_base(state) else {
        state.admin_console.registry_status = "Connect to the server first.".into();
        return;
    };
    track_server(state, &base);
    let tx = state.admin_console.tx.clone();
    state.admin_console.in_flight += 1;
    state.admin_console.registry_status = "Loading...".into();
    std::thread::spawn(move || {
        let result = (|| -> Result<crate::relay::admin::Registry, String> {
            let body = match ureq::get(&format!("{base}/api/admin/actions")).call() {
                Ok(r) => r.into_string().map_err(|e| format!("The list could not be read: {e}"))?,
                Err(ureq::Error::Status(404, _)) => return Err("This server is older than its list of admin actions: update it to see them here.".into()),
                Err(ureq::Error::Status(code, _)) => return Err(format!("The server answered {code}.")),
                Err(e) => return Err(format!("Could not reach the server: {e}")),
            };
            serde_json::from_str(&body).map_err(|e| format!("The server's list could not be read ({e})."))
        })();
        let _ = tx.send(AdminReply::Registry { server: base, result });
    });
}

/// Run `action` with `inputs` on the connected server, signed; `confirm` is a destructive
/// action's confirm word.
pub(crate) fn run_action(state: &mut GuiState, action: &str, inputs: Value, confirm: Option<String>) {
    let Some(base) = server_base(state) else {
        note(state, action, false, "Connect to the server first.".into());
        return;
    };
    track_server(state, &base);
    let key = state.profile_public_key.clone();
    let seed = state.private_key_bytes.clone();
    let tx = state.admin_console.tx.clone();
    state.admin_console.in_flight += 1;
    let action = action.to_string();
    std::thread::spawn(move || {
        let result = (|| -> Result<Value, String> {
            let seed = seed.ok_or_else(|| "No identity loaded: sign in first.".to_string())?;
            if key.is_empty() {
                return Err("No identity loaded: sign in first.".into());
            }
            let inputs_text = inputs.to_string();
            let ts = next_timestamp();
            // The Dilithium3 signature is CPU work: it is made here, on the worker, not the UI.
            let sig = crate::net::identity::pq_sign_chat(&seed, &crate::relay::admin::run_purpose(&action, &inputs_text), ts);
            let body = json!({ "key": key, "timestamp": ts, "sig": sig, "action": action, "inputs": inputs_text, "confirm": confirm }).to_string();
            match ureq::post(&format!("{base}/api/admin/actions/run")).set("Content-Type", "application/json").send_string(&body) {
                Ok(r) => serde_json::from_str(&r.into_string().map_err(|e| format!("The answer could not be read: {e}"))?)
                    .map_err(|e| format!("The answer could not be read: {e}")),
                Err(ureq::Error::Status(code, r)) => {
                    let v: Value = serde_json::from_str(&r.into_string().unwrap_or_default()).unwrap_or(Value::Null);
                    Err(v["error"].as_str().map(str::to_string).unwrap_or_else(|| {
                        if code == 404 { "This server is older than the admin actions: update it first.".to_string() } else { format!("The server answered {code}.") }
                    }))
                }
                Err(e) => Err(format!("Could not reach the server: {e}")),
            }
        })();
        let _ = tx.send(AdminReply::Ran { server: base, action, result });
    });
}

/// Put an action's sentence where its panel shows it.
fn note(state: &mut GuiState, action: &str, ok: bool, message: String) {
    // A list answers with what the panel then draws, so its sentence would only repeat it (the
    // website does the same, web/pages/admin-console.js `run`).
    let message = if ok && action.ends_with(".list") { String::new() } else { message };
    let c = &mut state.admin_console;
    if action.starts_with("alerts.") {
        c.alerts_status = (ok, message.clone());
    } else if action.starts_with("federation.") {
        c.federation_status = (ok, message.clone());
    } else if action == "log.recent" {
        c.log_status = message.clone();
    }
    c.results.insert(action.to_string(), (ok, message));
}

/// Take in every answer that has arrived. Called each frame a console panel is drawn.
pub(crate) fn drain(state: &mut GuiState, ctx: &egui::Context) {
    loop {
        let reply = match state.admin_console.rx.try_recv() {
            Ok(r) => r,
            Err(_) => break,
        };
        let c = &mut state.admin_console;
        c.in_flight = c.in_flight.saturating_sub(1);
        match reply {
            AdminReply::Registry { server, result } => {
                if server != c.server {
                    continue;
                }
                match result {
                    Ok(r) => {
                        c.registry = Some(r);
                        c.registry_status.clear();
                    }
                    Err(e) => c.registry_status = e,
                }
            }
            AdminReply::Ran { server, action, result } => {
                if server != c.server {
                    continue;
                }
                let (ok, message) = match &result {
                    Ok(v) => (true, v["message"].as_str().unwrap_or("Done.").to_string()),
                    Err(e) => (false, e.clone()),
                };
                if let Ok(v) = &result {
                    let data = &v["data"];
                    if action.starts_with("alerts.") && data.get("channels").is_some() {
                        c.alerts = Some(data.clone());
                        if action != "alerts.list" {
                            c.alert_edit = None;
                        }
                    }
                    if action.starts_with("federation.") && data.get("peers").is_some() {
                        c.federation = Some(data.clone());
                    }
                    if action == "log.recent" {
                        c.log = Some(data["entries"].as_array().cloned().unwrap_or_default());
                    }
                }
                // A change refreshes the admin log if it is open.
                let changed = ok && !action.ends_with(".list") && action != "log.recent";
                note(state, &action, ok, message);
                if changed && state.admin_console.log.is_some() {
                    run_action(state, "log.recent", json!({}), None);
                }
            }
        }
    }
    if state.admin_console.in_flight > 0 {
        ctx.request_repaint_after(Duration::from_millis(200));
    }
}

/// "3 minutes ago" for a time in milliseconds since 1970.
pub(crate) fn time_ago(ms: i64) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    let s = ((now - ms) / 1000).max(0);
    match s {
        0..=59 => "just now".into(),
        60..=3_599 => format!("{} min ago", s / 60),
        3_600..=86_399 => format!("{} h ago", s / 3_600),
        _ => format!("{} days ago", s / 86_400),
    }
}

/// "3 h" for a span in seconds.
pub(crate) fn span_words(secs: u64) -> String {
    match secs {
        0..=59 => format!("{secs} s"),
        60..=3_599 => format!("{} min", secs / 60),
        3_600..=86_399 => format!("{} h", secs / 3_600),
        _ => format!("{} days", secs / 86_400),
    }
}
