//! The dev MCP endpoint's HTTP side: the Streamable HTTP transport's POST, on 127.0.0.1 only.
//!
//! Small on purpose (std::net, one thread per connection, one JSON answer per POST, the
//! connection closed after it): a dev tool that one client at a time drives. What it checks
//! before the protocol sees a request, each for a reason:
//!
//! - **Loopback only.** `bind_loopback` refuses any address that is not this computer's, so the
//!   endpoint can never be reached from the network, and a listener on 127.0.0.1 also raises no
//!   Windows Firewall prompt (CLAUDE.md, "No Windows Firewall prompts from dev work").
//! - **Host and Origin** must name this computer (the specification's MUST: a web page on
//!   another site could otherwise reach a local server by DNS rebinding).
//! - **A bearer token** written only to the endpoint file (`debug/mcp_endpoint.json`): a page in a
//!   browser cannot read that file, so it cannot drive the game even from 127.0.0.1.
//! - **POST only** (the 2026-07-28 revision has no GET stream; GET and DELETE answer 405 as it
//!   says), JSON only, bounded sizes and read times.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use super::protocol::{self, Headers, ToolHost};

/// The one path the endpoint answers on.
pub(crate) const MCP_PATH: &str = "/mcp";
/// The longest request head (request line and headers) read, bytes.
const MAX_HEAD_BYTES: usize = 16 * 1024;
/// The longest request body read, bytes. A tool call is a few hundred.
const MAX_BODY_BYTES: usize = 1024 * 1024;
/// Connections handled at once; more are answered 503 and closed.
const MAX_OPEN: usize = 8;
/// How long a client may take to send its request.
const READ_TIMEOUT: Duration = Duration::from_secs(10);
/// How long writing the answer may take.
const WRITE_TIMEOUT: Duration = Duration::from_secs(30);

/// Listen on `addr`, which must be on this computer. Anything else (0.0.0.0, a network address)
/// is refused before a socket exists: the endpoint drives a game, and nothing off this computer
/// may ever reach it.
pub(crate) fn bind_loopback(addr: SocketAddr) -> Result<TcpListener, String> {
    if !addr.ip().is_loopback() {
        return Err(format!(
            "the dev MCP endpoint listens on this computer only (127.0.0.1): refusing to listen on {addr}"
        ));
    }
    TcpListener::bind(addr).map_err(|e| format!("could not listen on {addr}: {e}"))
}

/// One HTTP request as read off the socket.
#[derive(Debug, Default, Clone)]
pub(crate) struct Request {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    /// A header's value by name, case-insensitively (HTTP names are).
    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

/// One HTTP answer.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Response {
    pub status: u16,
    /// A JSON body, or none (202).
    pub body: Option<Value>,
    pub allow_post_only: bool,
}

impl Response {
    fn json(status: u16, body: Value) -> Self {
        Response { status, body: Some(body), allow_post_only: false }
    }
    /// A refusal before the protocol: a JSON-RPC error with no id, as the transport allows.
    fn refuse(status: u16, message: impl Into<String>) -> Self {
        Response::json(status, json!({ "jsonrpc": "2.0", "error": { "code": protocol::INVALID_REQUEST, "message": message.into() } }))
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        100 => "Continue",
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        411 => "Length Required",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        431 => "Request Header Fields Too Large",
        503 => "Service Unavailable",
        _ => "Error",
    }
}

/// Is `value` (a Host header, or an Origin with its scheme taken off) this computer at `port`?
fn names_this_computer(value: &str, port: u16) -> bool {
    let v = value.trim().to_ascii_lowercase();
    ["127.0.0.1", "localhost", "[::1]"].iter().any(|h| v == format!("{h}:{port}"))
}

/// Compare two tokens without stopping at the first difference.
fn same_token(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// The checks every request passes before the protocol sees it (see the module comment). Err is
/// the answer to send instead.
pub(crate) fn admit(req: &Request, port: u16, token: &str) -> Result<(), Response> {
    let path = req.path.split('?').next().unwrap_or("");
    if path != MCP_PATH {
        return Err(Response::refuse(404, format!("the MCP endpoint is {MCP_PATH}")));
    }
    match req.header("host") {
        Some(h) if names_this_computer(h, port) => {}
        Some(h) => return Err(Response::refuse(403, format!("Host {h:?} is not this computer: refused (DNS rebinding guard)"))),
        None => return Err(Response::refuse(403, "a request without a Host header is refused")),
    }
    if let Some(origin) = req.header("origin") {
        let ok = origin.strip_prefix("http://").is_some_and(|rest| names_this_computer(rest, port));
        if !ok {
            return Err(Response::refuse(403, format!("Origin {origin:?} is not this computer: refused")));
        }
    }
    if req.method != "POST" {
        let mut r = Response::refuse(405, "the endpoint takes POST only (the 2026-07-28 revision has no GET stream)");
        r.allow_post_only = true;
        return Err(r);
    }
    let presented = req.header("authorization").and_then(|a| a.strip_prefix("Bearer ")).map(str::trim);
    if !presented.is_some_and(|t| same_token(t, token)) {
        return Err(Response::refuse(
            401,
            "missing or wrong bearer token: read `token` from the game's debug/mcp_endpoint.json (scripts/mcp-server.js does)",
        ));
    }
    let ct = req.header("content-type").unwrap_or("").to_ascii_lowercase();
    if !ct.starts_with("application/json") {
        return Err(Response::refuse(415, "the body must be application/json"));
    }
    Ok(())
}

/// The protocol's view of a request's headers.
fn protocol_headers(req: &Request) -> Headers {
    Headers {
        protocol_version: req.header("mcp-protocol-version").map(|s| s.trim().to_string()),
        method: req.header("mcp-method").map(|s| s.trim().to_string()),
        name: req.header("mcp-name").and_then(|s| protocol::decode_header_value(s.trim())),
    }
}

/// Answer one request: the checks, then the protocol.
pub(crate) fn respond(req: &Request, port: u16, token: &str, host: &dyn ToolHost) -> Response {
    if let Err(r) = admit(req, port, token) {
        return r;
    }
    let msg: Value = match serde_json::from_slice(&req.body) {
        Ok(v) => v,
        Err(e) => {
            return Response::json(
                400,
                json!({ "jsonrpc": "2.0", "id": null, "error": { "code": protocol::PARSE_ERROR, "message": format!("the body is not JSON: {e}") } }),
            )
        }
    };
    let reply = protocol::handle(&msg, Some(&protocol_headers(req)), host);
    Response { status: reply.status, body: reply.body, allow_post_only: false }
}

/// Read one request off the socket. Err is the answer to send (a malformed or oversized request).
pub(crate) fn read_request(stream: &mut impl ReadWrite) -> Result<Request, Response> {
    let mut buf: Vec<u8> = Vec::with_capacity(2048);
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        if buf.len() > MAX_HEAD_BYTES {
            return Err(Response::refuse(431, "request head too large"));
        }
        let n = stream.read(&mut chunk).map_err(|e| Response::refuse(400, format!("reading the request: {e}")))?;
        if n == 0 {
            return Err(Response::refuse(400, "the connection closed before the request was whole"));
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let (Some(method), Some(path)) = (parts.next(), parts.next()) else {
        return Err(Response::refuse(400, "malformed request line"));
    };
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':').map(|(k, v)| (k.trim().to_string(), v.trim().to_string())))
        .collect();
    let mut req = Request { method: method.to_string(), path: path.to_string(), headers, body: Vec::new() };
    if req.header("transfer-encoding").is_some_and(|t| t.to_ascii_lowercase().contains("chunked")) {
        return Err(Response::refuse(411, "send the body with a Content-Length (chunked bodies are not read)"));
    }
    let len = match req.header("content-length") {
        Some(l) => l.trim().parse::<usize>().map_err(|_| Response::refuse(400, "Content-Length is not a number"))?,
        None => 0,
    };
    if len > MAX_BODY_BYTES {
        return Err(Response::refuse(413, format!("the body is over {MAX_BODY_BYTES} bytes")));
    }
    if len > 0 && req.header("expect").is_some_and(|e| e.eq_ignore_ascii_case("100-continue")) {
        let _ = stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n");
    }
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < len {
        let n = stream.read(&mut chunk).map_err(|e| Response::refuse(400, format!("reading the body: {e}")))?;
        if n == 0 {
            return Err(Response::refuse(400, "the connection closed before the body was whole"));
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(len);
    req.body = body;
    Ok(req)
}

/// The bytes of an answer.
pub(crate) fn encode_response(r: &Response) -> Vec<u8> {
    let body = r.body.as_ref().map(|b| b.to_string()).unwrap_or_default();
    let mut head = format!("HTTP/1.1 {} {}\r\nConnection: close\r\nCache-Control: no-store\r\n", r.status, reason(r.status));
    if r.allow_post_only {
        head.push_str("Allow: POST\r\n");
    }
    if r.body.is_some() {
        head.push_str("Content-Type: application/json\r\n");
    }
    head.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));
    let mut out = head.into_bytes();
    out.extend_from_slice(body.as_bytes());
    out
}

/// What a connection is read from and written to (a TcpStream; the tests hand in a buffer).
pub(crate) trait ReadWrite: Read + Write {}
impl<T: Read + Write> ReadWrite for T {}

fn handle_connection(mut stream: TcpStream, port: u16, token: &str, host: &dyn ToolHost) {
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
    let answer = match read_request(&mut stream) {
        Ok(req) => respond(&req, port, token, host),
        Err(r) => r,
    };
    let _ = stream.write_all(&encode_response(&answer));
    let _ = stream.flush();
}

/// Serve `listener` on a thread of its own until the process ends: each connection on a thread,
/// at most MAX_OPEN at once.
pub(crate) fn serve(listener: TcpListener, token: String, host: Arc<dyn ToolHost + Send + Sync>) -> std::io::Result<()> {
    let port = listener.local_addr()?.port();
    let open = Arc::new(AtomicUsize::new(0));
    std::thread::Builder::new().name("dev-mcp-accept".into()).spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut stream) = conn else { continue };
            if open.load(Ordering::SeqCst) >= MAX_OPEN {
                let _ = stream.write_all(&encode_response(&Response::refuse(503, "too many connections at once")));
                continue;
            }
            open.fetch_add(1, Ordering::SeqCst);
            let (open_c, token, host) = (open.clone(), token.clone(), host.clone());
            let spawned = std::thread::Builder::new().name("dev-mcp-conn".into()).spawn(move || {
                handle_connection(stream, port, &token, &*host);
                open_c.fetch_sub(1, Ordering::SeqCst);
            });
            if spawned.is_err() {
                open.fetch_sub(1, Ordering::SeqCst);
                log::warn!("Dev MCP: could not start a connection thread");
            }
        }
    })?;
    Ok(())
}
