//! HTTP API v2: agent coordination dashboard endpoints (v0.118.0).
//!
//! Surfaces the multi-agent state for a frontend dashboard:
//!   - GET /api/v2/agents/status
//!     → { registry: [...], sessions: [...], runtime: [...], generated_at }
//!     Single one-shot endpoint that returns:
//!       - registry  : parsed scope ownership from data/coordination/agent_registry.ron
//!       - sessions  : audit JSON files in data/coordination/sessions/*.json
//!       - runtime   : live agent_sessions table rows (claim/heartbeat state)
//!       - overrides : user-set status overrides from data/coordination/overrides.ron (if present)
//!
//!   - GET /api/v2/agents/sessions
//!     → just the runtime claims table
//!
//!   - POST /api/v2/agents/override { scope_id, status }
//!     → set a user override for a scope's status (active/passive/blocked)
//!
//! The dashboard page (/agents) hits these endpoints to render a live
//! spreadsheet of every scope and its state.

use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

use crate::relay::relay::RelayState;

#[derive(Debug, Serialize, Deserialize)]
pub struct OverrideRequest {
    pub scope_id: String,
    pub status: String,
}

/// `GET /api/v2/agents/status` — one-shot aggregate for the dashboard.
pub async fn get_agents_status(
    State(state): State<Arc<RelayState>>,
) -> impl IntoResponse {
    let runtime_sessions = state.db.agent_list_sessions().unwrap_or_default();

    // Read registry RON file. Best-effort — if the file is missing or malformed,
    // return the runtime sessions and an empty registry so the UI degrades gracefully.
    let registry_text = std::fs::read_to_string("data/coordination/agent_registry.ron")
        .unwrap_or_default();
    let registry_scopes = parse_registry_scopes(&registry_text);

    // Read every session JSON file. Same degrade-gracefully principle.
    let session_dir = PathBuf::from("data/coordination/sessions");
    let mut session_files: Vec<serde_json::Value> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&session_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                        session_files.push(val);
                    }
                }
            }
        }
    }

    // Optional user overrides file
    let overrides_text = std::fs::read_to_string("data/coordination/overrides.ron")
        .unwrap_or_default();
    let overrides = parse_overrides(&overrides_text);

    let body = serde_json::json!({
        "registry": registry_scopes,
        "sessions": session_files,
        "runtime": runtime_sessions,
        "overrides": overrides,
        "generated_at": super::storage::now_millis(),
    });
    (StatusCode::OK, Json(body)).into_response()
}

/// `GET /api/v2/agents/sessions` — runtime claims only.
pub async fn list_agent_sessions(
    State(state): State<Arc<RelayState>>,
) -> impl IntoResponse {
    match state.db.agent_list_sessions() {
        Ok(rows) => (StatusCode::OK, Json(serde_json::to_value(rows).unwrap_or_default())).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("{e}")}))).into_response(),
    }
}

/// `POST /api/v2/agents/override` — user override for a scope's effective status.
/// Writes to data/coordination/overrides.ron. The dashboard reflects this
/// immediately on the next /api/v2/agents/status read.
pub async fn set_agent_override(
    State(state): State<Arc<RelayState>>,
    Json(req): Json<OverrideRequest>,
) -> impl IntoResponse {
    if !["active", "passive", "blocked", "off"].contains(&req.status.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "status must be one of: active, passive, blocked, off"})),
        )
            .into_response();
    }

    let path = PathBuf::from("data/coordination/overrides.ron");
    let existing_text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut entries = parse_overrides(&existing_text);
    entries.retain(|(s, _)| s != &req.scope_id);
    entries.push((req.scope_id.clone(), req.status.clone()));

    let mut out = String::from("// User overrides for scope status. Edited by\n");
    out.push_str("// POST /api/v2/agents/override or by hand. Hot-reloadable.\n");
    out.push_str("(\n    overrides: [\n");
    for (scope_id, status) in &entries {
        out.push_str(&format!("        ( scope_id: \"{}\", status: \"{}\" ),\n", scope_id, status));
    }
    out.push_str("    ],\n)\n");

    if let Err(e) = std::fs::write(&path, out) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("write failed: {e}")})),
        )
            .into_response();
    }

    // Announce the override on the announcements channel. Best-effort —
    // never blocks or fails the override.
    let msg = format!(
        "Agent override: scope `{}` → `{}`",
        req.scope_id, req.status
    );
    crate::relay::handlers::announce::announce_async(
        state.clone(),
        crate::relay::handlers::announce::DEFAULT_ANNOUNCEMENT_CHANNEL.to_string(),
        msg,
    );

    (StatusCode::OK, Json(serde_json::json!({"ok": true, "scope_id": req.scope_id, "status": req.status}))).into_response()
}

// ── tiny RON parsers (good enough for our shape; full ron crate too heavy here) ──

#[derive(Debug, Serialize)]
struct RegistryScope {
    id: String,
    owns: Vec<String>,
    must_not_touch: Vec<String>,
    completion_check: String,
    default_status: String,
}

/// Parse the agent_registry.ron file's scope entries cheaply via line scanning.
/// Tolerant: malformed entries are skipped, never panic.
fn parse_registry_scopes(text: &str) -> Vec<RegistryScope> {
    let mut scopes = Vec::new();
    let mut current: Option<RegistryScope> = None;
    let mut in_owns = false;
    let mut in_must = false;

    for raw in text.lines() {
        let line = raw.trim();

        if let Some(id) = extract_quoted_field(line, "id:") {
            // New scope start — flush prior, begin fresh
            if let Some(c) = current.take() {
                scopes.push(c);
            }
            current = Some(RegistryScope {
                id,
                owns: Vec::new(),
                must_not_touch: Vec::new(),
                completion_check: String::new(),
                default_status: String::new(),
            });
            continue;
        }
        if current.is_none() {
            continue;
        }

        if line.starts_with("owns:") {
            in_owns = true;
            in_must = false;
            // collect any inline strings on this line
            for s in extract_quoted_strings(line) {
                if let Some(c) = current.as_mut() { c.owns.push(s); }
            }
            if line.contains("]") { in_owns = false; }
            continue;
        }
        if line.starts_with("must_not_touch:") {
            in_must = true;
            in_owns = false;
            for s in extract_quoted_strings(line) {
                if let Some(c) = current.as_mut() { c.must_not_touch.push(s); }
            }
            if line.contains("]") { in_must = false; }
            continue;
        }

        if in_owns {
            for s in extract_quoted_strings(line) {
                if let Some(c) = current.as_mut() { c.owns.push(s); }
            }
            if line.contains("]") { in_owns = false; }
            continue;
        }
        if in_must {
            for s in extract_quoted_strings(line) {
                if let Some(c) = current.as_mut() { c.must_not_touch.push(s); }
            }
            if line.contains("]") { in_must = false; }
            continue;
        }

        if let Some(s) = extract_quoted_field(line, "completion_check:") {
            if let Some(c) = current.as_mut() { c.completion_check = s; }
            continue;
        }
        if let Some(s) = extract_quoted_field(line, "default_status:") {
            if let Some(c) = current.as_mut() { c.default_status = s; }
            continue;
        }
    }
    if let Some(c) = current.take() {
        scopes.push(c);
    }
    scopes
}

fn parse_overrides(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if !line.starts_with("(") || !line.contains("scope_id") {
            continue;
        }
        let scope = extract_quoted_field(line, "scope_id:").unwrap_or_default();
        let status = extract_quoted_field(line, "status:").unwrap_or_default();
        if !scope.is_empty() && !status.is_empty() {
            out.push((scope, status));
        }
    }
    out
}

/// Extract the first quoted-string value following a key like `id: "value"`.
fn extract_quoted_field(line: &str, key: &str) -> Option<String> {
    let pos = line.find(key)?;
    let rest = &line[pos + key.len()..];
    let start = rest.find('"')? + 1;
    let after = &rest[start..];
    let end = after.find('"')?;
    Some(after[..end].to_string())
}

/// Extract every "..." string in a line (used for inline arrays).
fn extract_quoted_strings(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() && bytes[j] != b'"' {
                j += 1;
            }
            if j < bytes.len() {
                if let Ok(s) = std::str::from_utf8(&bytes[start..j]) {
                    out.push(s.to_string());
                }
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_simple_scope_block() {
        let ron = r#"
            (
                id: "elements",
                owns: ["data/chemistry/elements.csv"],
                must_not_touch: ["src/"],
                completion_check: "Periodic table done.",
                default_status: "passive",
            ),
        "#;
        let scopes = parse_registry_scopes(ron);
        assert_eq!(scopes.len(), 1);
        assert_eq!(scopes[0].id, "elements");
        assert_eq!(scopes[0].owns, vec!["data/chemistry/elements.csv"]);
        assert_eq!(scopes[0].default_status, "passive");
    }

    #[test]
    fn parses_multi_scope() {
        let ron = r#"
            (
                id: "a",
                owns: ["x"],
                must_not_touch: [],
                completion_check: "first",
                default_status: "active",
            ),
            (
                id: "b",
                owns: ["y", "z"],
                must_not_touch: ["w"],
                completion_check: "second",
                default_status: "passive",
            ),
        "#;
        let scopes = parse_registry_scopes(ron);
        assert_eq!(scopes.len(), 2);
        assert_eq!(scopes[0].id, "a");
        assert_eq!(scopes[1].id, "b");
        assert_eq!(scopes[1].owns, vec!["y", "z"]);
    }

    #[test]
    fn parses_overrides() {
        let ron = r#"
            (
                overrides: [
                    ( scope_id: "compounds", status: "active" ),
                    ( scope_id: "elements", status: "off" ),
                ],
            )
        "#;
        let parsed = parse_overrides(ron);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0], ("compounds".to_string(), "active".to_string()));
    }
}
