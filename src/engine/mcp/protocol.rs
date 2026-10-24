//! The Model Context Protocol subset the dev endpoint speaks: JSON-RPC 2.0 with `tools/list` and
//! `tools/call`, in both eras the specification has had (read 2026-10-22 from
//! modelcontextprotocol.io and the schema.ts of each revision):
//!
//! - LEGACY (2025-11-25 and earlier): the client opens with `initialize`, the server answers with
//!   the version both speak, then `notifications/initialized`, `ping`, `tools/list`, `tools/call`.
//! - MODERN (2026-07-28): no handshake. Every request carries its version in
//!   `params._meta["io.modelcontextprotocol/protocolVersion"]`; the server answers
//!   `server/discover`, refuses a version it does not speak with -32022 (listing the ones it
//!   does), and every result carries `resultType`. On HTTP the version, the method and a tool
//!   call's name are mirrored into headers, which must agree with the body (-32020).
//!
//! A "dual-era" server answers both, chosen per request by how the client speaks, which is what
//! this does: Claude Code and the scripted clients get the same tools whichever era they are in.
//! Pure (no sockets, no engine), so the tests drive it directly. The node bridge
//! (scripts/lib/mcp-protocol.js) is the same rules for the stdio side.

use serde_json::{json, Value};

/// The modern revision this speaks.
pub(crate) const MODERN_VERSIONS: &[&str] = &["2026-07-28"];
/// The handshake revisions this speaks, newest first (the one offered to a client asking for a
/// version not listed).
pub(crate) const LEGACY_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// JSON-RPC and MCP error codes used here.
pub(crate) const PARSE_ERROR: i64 = -32700;
pub(crate) const INVALID_REQUEST: i64 = -32600;
pub(crate) const METHOD_NOT_FOUND: i64 = -32601;
pub(crate) const INVALID_PARAMS: i64 = -32602;
pub(crate) const HEADER_MISMATCH: i64 = -32020;
pub(crate) const UNSUPPORTED_PROTOCOL_VERSION: i64 = -32022;

/// The `_meta` key a modern request names its version under.
pub(crate) const META_VERSION: &str = "io.modelcontextprotocol/protocolVersion";
/// The `_meta` key a modern result names its server under.
pub(crate) const META_SERVER_INFO: &str = "io.modelcontextprotocol/serverInfo";

/// What the server is and offers. The endpoint's implementation is the game's tools
/// (engine/mcp/tools.rs); the tests hand in a double.
pub(crate) trait ToolHost {
    /// Every tool's definition, in a fixed order.
    fn tools(&self) -> Vec<Value>;
    /// Run a tool. None: no tool has that name. Some: a CallToolResult (`content`, `isError`,
    /// `structuredContent`), errors of the tool's own included.
    fn call(&self, name: &str, args: &Value) -> Option<Value>;
    /// `{name, version}` for `serverInfo`.
    fn server_info(&self) -> Value;
    /// A few sentences on how to use the tools, for the client's model.
    fn instructions(&self) -> String;
}

/// The HTTP headers a request arrived with that the protocol checks (none on stdio).
#[derive(Debug, Default, Clone)]
pub(crate) struct Headers {
    pub protocol_version: Option<String>,
    pub method: Option<String>,
    /// `Mcp-Name`, already decoded from the `=?base64?...?=` form when it came that way.
    pub name: Option<String>,
}

/// What to send back: an HTTP status (stdio ignores it) and a body, or no body (a notification
/// taken, HTTP 202).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Reply {
    pub status: u16,
    pub body: Option<Value>,
}

fn error(id: &Value, status: u16, code: i64, message: impl Into<String>, data: Option<Value>) -> Reply {
    let mut e = json!({ "code": code, "message": message.into() });
    if let Some(d) = data {
        e["data"] = d;
    }
    Reply { status, body: Some(json!({ "jsonrpc": "2.0", "id": id, "error": e })) }
}

fn result(id: &Value, r: Value) -> Reply {
    Reply { status: 200, body: Some(json!({ "jsonrpc": "2.0", "id": id, "result": r })) }
}

/// A modern result's `_meta`: the server's name and version.
fn server_meta(host: &dyn ToolHost) -> Value {
    let mut m = serde_json::Map::new();
    m.insert(META_SERVER_INFO.to_string(), host.server_info());
    Value::Object(m)
}

/// Every version this answers, modern first.
pub(crate) fn supported_versions() -> Vec<&'static str> {
    MODERN_VERSIONS.iter().chain(LEGACY_VERSIONS.iter()).copied().collect()
}

/// The legacy version to answer `initialize` with: the client's own when this speaks it, else the
/// newest legacy one (the client then decides whether it can go on).
pub(crate) fn negotiate_legacy(asked: Option<&str>) -> &'static str {
    asked
        .and_then(|a| LEGACY_VERSIONS.iter().find(|v| **v == a).copied())
        .unwrap_or(LEGACY_VERSIONS[0])
}

/// Decode a header value that may be in the `=?base64?...?=` form (2026-07-28 Value Encoding).
pub(crate) fn decode_header_value(raw: &str) -> Option<String> {
    use base64::Engine as _;
    if let Some(inner) = raw.strip_prefix("=?base64?").and_then(|r| r.strip_suffix("?=")) {
        let bytes = base64::engine::general_purpose::STANDARD.decode(inner).ok()?;
        return String::from_utf8(bytes).ok();
    }
    Some(raw.to_string())
}

/// Handle one JSON-RPC message. `headers` is Some on HTTP (checked against the body), None on
/// stdio.
pub(crate) fn handle(msg: &Value, headers: Option<&Headers>, host: &dyn ToolHost) -> Reply {
    let null = Value::Null;
    let Some(obj) = msg.as_object() else {
        return error(&null, 400, INVALID_REQUEST, "a JSON-RPC message must be one JSON object (batches are not part of MCP)", None);
    };
    if obj.get("jsonrpc").and_then(|v| v.as_str()) != Some("2.0") {
        return error(obj.get("id").unwrap_or(&null), 400, INVALID_REQUEST, "jsonrpc must be \"2.0\"", None);
    }
    let Some(method) = obj.get("method").and_then(|m| m.as_str()) else {
        // A response or garbage: this server never sends requests, so nothing awaits a response.
        return error(obj.get("id").unwrap_or(&null), 400, INVALID_REQUEST, "expected a request or a notification (a message with a method)", None);
    };
    let params = obj.get("params").cloned().unwrap_or_else(|| json!({}));
    let Some(id) = obj.get("id") else {
        // A notification (initialized, cancelled, ...): taken, nothing to say.
        return Reply { status: 202, body: None };
    };
    if !(id.is_string() || id.is_i64() || id.is_u64()) {
        return error(&null, 400, INVALID_REQUEST, "a request id must be a string or an integer", None);
    }
    let modern_version = params.pointer("/_meta").and_then(|m| m.get(META_VERSION)).and_then(|v| v.as_str());

    // ── The HTTP headers must agree with the body (2026-07-28 Server Validation). ──
    if let Some(h) = headers {
        if let Some(hm) = h.method.as_deref() {
            if hm != method {
                return error(id, 400, HEADER_MISMATCH, format!("Header mismatch: Mcp-Method header value '{hm}' does not match body value '{method}'"), None);
            }
        }
        if let Some(v) = modern_version {
            match h.protocol_version.as_deref() {
                Some(hv) if hv == v => {}
                Some(hv) => {
                    return error(id, 400, HEADER_MISMATCH, format!("Header mismatch: MCP-Protocol-Version header value '{hv}' does not match body value '{v}'"), None)
                }
                None => return error(id, 400, HEADER_MISMATCH, "Header mismatch: the MCP-Protocol-Version header is missing", None),
            }
            if h.method.is_none() {
                return error(id, 400, HEADER_MISMATCH, "Header mismatch: the Mcp-Method header is missing", None);
            }
            if method == "tools/call" {
                let body_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                match h.name.as_deref() {
                    Some(hn) if hn == body_name => {}
                    Some(hn) => {
                        return error(id, 400, HEADER_MISMATCH, format!("Header mismatch: Mcp-Name header value '{hn}' does not match body value '{body_name}'"), None)
                    }
                    None => return error(id, 400, HEADER_MISMATCH, "Header mismatch: the Mcp-Name header is missing", None),
                }
            }
        }
    }

    // ── Modern: the version must be one this speaks. ──
    if let Some(v) = modern_version {
        if !MODERN_VERSIONS.contains(&v) {
            return error(
                id,
                400,
                UNSUPPORTED_PROTOCOL_VERSION,
                "Unsupported protocol version",
                Some(json!({ "supported": supported_versions(), "requested": v })),
            );
        }
    }
    let modern = modern_version.is_some();
    // A modern result names its type and its server; a legacy one is left as that revision had it.
    let finish = |mut r: Value| -> Value {
        if modern {
            r["resultType"] = json!("complete");
            r["_meta"] = server_meta(host);
        }
        r
    };

    match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(|v| v.as_str());
            result(
                id,
                json!({
                    "protocolVersion": negotiate_legacy(asked),
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": host.server_info(),
                    "instructions": host.instructions(),
                }),
            )
        }
        "ping" => result(id, finish(json!({}))),
        "server/discover" => result(
            id,
            json!({
                "resultType": "complete",
                "supportedVersions": supported_versions(),
                "capabilities": { "tools": {} },
                "instructions": host.instructions(),
                "ttlMs": 0,
                "cacheScope": "private",
                "_meta": server_meta(host),
            }),
        ),
        "tools/list" => {
            let mut r = json!({ "tools": host.tools() });
            if modern {
                r["ttlMs"] = json!(0);
                r["cacheScope"] = json!("private");
            }
            result(id, finish(r))
        }
        "tools/call" => {
            let Some(name) = params.get("name").and_then(|n| n.as_str()) else {
                return error(id, 200, INVALID_PARAMS, "tools/call needs params.name, the tool's name", None);
            };
            let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            if !args.is_object() {
                return error(id, 200, INVALID_PARAMS, "tools/call params.arguments must be an object", None);
            }
            match host.call(name, &args) {
                Some(r) => result(id, finish(r)),
                None => error(id, 200, INVALID_PARAMS, format!("Unknown tool: {name}"), None),
            }
        }
        other => error(
            id,
            if modern { 404 } else { 200 },
            METHOD_NOT_FOUND,
            format!("Method not found: {other} (this server offers tools only: tools/list, tools/call)"),
            None,
        ),
    }
}

/// A tool's answer: text, optionally with structured content.
pub(crate) fn text_result(text: impl Into<String>, structured: Option<Value>) -> Value {
    let mut r = json!({ "content": [{ "type": "text", "text": text.into() }], "isError": false });
    if let Some(s) = structured {
        r["structuredContent"] = s;
    }
    r
}

/// A tool's own failure, said plainly (the model reads it and can correct itself).
pub(crate) fn tool_error(text: impl Into<String>) -> Value {
    json!({ "content": [{ "type": "text", "text": text.into() }], "isError": true })
}
