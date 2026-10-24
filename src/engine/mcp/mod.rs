//! The dev MCP endpoint (2026-10-22): the running game serves the Model Context Protocol on
//! 127.0.0.1, so an AI session drives and inspects it as tools instead of dropping files into
//! `debug/` and polling for answers. Claude Code reaches it through scripts/mcp-server.js (a
//! stdio bridge that also holds the tools that need no game: the rigs, the lib tests, the
//! journal), which finds the port and token in `debug/mcp_endpoint.json`. docs/ai/mcp.md.
//!
//! **Every tool is a thin call into the dev IPC** (engine/ipc.rs): its request is handed to the
//! one request funnel, `take_request_text`, through `take_injected`, and taken the next time its
//! poller runs, exactly as a dropped file is; the answer is the done file the handler writes. So
//! the file IPC and the MCP cannot diverge: there is one handler per request, and the rigs keep
//! dropping files at it.
//!
//! **Off unless asked, and only in a sandbox.** It starts when `HUMANITY_DEV_MCP=1` is set AND the
//! game runs portable (`portable.txt` beside the exe: its own identity, config and saves, never
//! the operator's) AND its server is on this computer (none, a throwaway relay, or the rigs' dead
//! loopback port). That is every rig and nothing a player runs: a player never sets the variable,
//! and an installed game is not portable. If the server ever changes to one off this computer,
//! every call is refused. It listens on 127.0.0.1 only (http.rs), behind a token.

mod http;
mod link;
mod protocol;
mod tools;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::engine::state::EngineState;

/// The development switch: the endpoint starts only when this is set (to anything but "0").
pub(crate) const ENV_SWITCH: &str = "HUMANITY_DEV_MCP";
/// Where the endpoint says where it is (and its token), in the game's working folder. A refusal
/// is written there too, so whoever set the switch learns why nothing listens.
pub(crate) const ENDPOINT_FILE: &str = "debug/mcp_endpoint.json";

/// Whether to start the endpoint.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Start {
    /// The switch is not set: nothing happens, nothing is logged.
    Off,
    /// The switch is set but the game is not a sandbox the endpoint may drive.
    Refused(String),
    /// Start it.
    Go,
}

/// Is the switch on? Any value but empty or "0".
pub(crate) fn switch_on(value: Option<&str>) -> bool {
    value.map(str::trim).is_some_and(|v| !v.is_empty() && v != "0")
}

/// Is a server address on this computer? An empty address is no server at all, which is on it.
/// Loopback names only: 127.x.x.x, localhost, ::1. Anything else, a LAN address included, is off
/// this computer.
pub(crate) fn server_is_local(url: &str) -> bool {
    let url = url.trim();
    if url.is_empty() {
        return true;
    }
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let authority = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = if let Some(v6) = authority.strip_prefix('[') {
        v6.split(']').next().unwrap_or("")
    } else {
        authority.split(':').next().unwrap_or("")
    };
    let host = host.to_ascii_lowercase();
    host == "localhost" || host == "::1" || host.parse::<std::net::Ipv4Addr>().is_ok_and(|ip| ip.is_loopback())
}

/// Whether to start, from the switch, the storage mode and the configured server.
pub(crate) fn decide(switch: Option<&str>, portable: bool, server_url: &str) -> Start {
    if !switch_on(switch) {
        return Start::Off;
    }
    if !portable {
        return Start::Refused(
            "this game is not a portable sandbox (no portable.txt beside the exe), so it runs on a real identity and \
             config; the endpoint only drives a rig's sandbox. Boot one with scripts/mcp-server.js game_boot or a rig."
                .to_string(),
        );
    }
    if !server_is_local(server_url) {
        return Start::Refused(format!(
            "this sandbox's server is {server_url:?}, off this computer; the endpoint only drives a game whose server is \
             on this computer (none, a throwaway relay, or the rigs' dead loopback port)"
        ));
    }
    Start::Go
}

// ── The funnel's other door: requests handed in by the endpoint ─────────────────────────────────

/// True once the endpoint runs: until then `take_injected` returns at its first line.
static ACTIVE: AtomicBool = AtomicBool::new(false);
/// Frames run since the game started (`frame_tick`).
static FRAME: AtomicU64 = AtomicU64::new(0);
/// Whether the game's server is on this computer, as of the last frame.
static SERVER_LOCAL: AtomicBool = AtomicBool::new(true);
static NEXT_TICKET: AtomicU64 = AtomicU64::new(1);

struct Pending {
    path: PathBuf,
    text: String,
    ticket: u64,
}

/// Requests handed in and not yet taken.
static PENDING: Mutex<Vec<Pending>> = Mutex::new(Vec::new());
/// Requests taken: (ticket, the frame they were taken in). Kept short: the waiting call reads
/// and removes its own.
static TAKEN: Mutex<Vec<(u64, u64)>> = Mutex::new(Vec::new());

/// The request handed in for `path`, if any: called first thing by the dev IPC's request funnel
/// (engine/ipc.rs `take_request_text_at`), so it is taken exactly as a dropped file would be.
/// One atomic load when the endpoint is not running.
pub(crate) fn take_injected(path: &Path) -> Option<String> {
    if !ACTIVE.load(Ordering::Relaxed) {
        return None;
    }
    let mut pending = PENDING.lock().unwrap_or_else(|p| p.into_inner());
    let i = pending.iter().position(|p| p.path == path)?;
    let p = pending.remove(i);
    drop(pending);
    let frame = FRAME.load(Ordering::SeqCst);
    let mut taken = TAKEN.lock().unwrap_or_else(|p| p.into_inner());
    taken.push((p.ticket, frame));
    // A call that gave up never reads its entry; keep the list from growing without bound.
    let excess = taken.len().saturating_sub(64);
    taken.drain(..excess);
    Some(p.text)
}

/// Hand a request in for the funnel's next take of `path`. Returns its ticket.
pub(crate) fn inject(path: &Path, text: String) -> u64 {
    let ticket = NEXT_TICKET.fetch_add(1, Ordering::SeqCst);
    let mut pending = PENDING.lock().unwrap_or_else(|p| p.into_inner());
    // A request for the same file that was never taken is replaced, as a second dropped file
    // would replace the first.
    pending.retain(|p| p.path != path);
    pending.push(Pending { path: path.to_path_buf(), text, ticket });
    ticket
}

/// The frame a ticket's request was taken in (once; the entry is removed).
pub(crate) fn taken_frame(ticket: u64) -> Option<u64> {
    let mut taken = TAKEN.lock().unwrap_or_else(|p| p.into_inner());
    let i = taken.iter().position(|t| t.0 == ticket)?;
    Some(taken.remove(i).1)
}

/// Take back a request not yet taken. False when it was taken meanwhile (or never existed).
pub(crate) fn withdraw(ticket: u64) -> bool {
    let mut pending = PENDING.lock().unwrap_or_else(|p| p.into_inner());
    let before = pending.len();
    pending.retain(|p| p.ticket != ticket);
    pending.len() != before
}

/// Frames run since the game started.
pub(crate) fn frames() -> u64 {
    FRAME.load(Ordering::SeqCst)
}

/// Whether the game's server was on this computer at the last frame.
pub(crate) fn server_local() -> bool {
    SERVER_LOCAL.load(Ordering::SeqCst)
}

// ── Once a frame ────────────────────────────────────────────────────────────────────────────

thread_local! {
    /// The server address last seen, so the check runs only when it changes.
    static LAST_SERVER: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Called once a frame from the main loop (lib.rs, beside the dev IPC's other unconditional
/// polls). The first call decides whether to start the endpoint; every call counts the frame and
/// notes whether the server is still on this computer.
pub(crate) fn frame_tick(state: &EngineState) {
    static DECIDED: std::sync::Once = std::sync::Once::new();
    DECIDED.call_once(|| start(state));
    if !ACTIVE.load(Ordering::Relaxed) {
        return;
    }
    FRAME.fetch_add(1, Ordering::SeqCst);
    let url = &state.gui_state.server_url;
    LAST_SERVER.with(|last| {
        let mut last = last.borrow_mut();
        if last.as_deref() != Some(url.as_str()) {
            let local = server_is_local(url);
            if !local && SERVER_LOCAL.load(Ordering::SeqCst) {
                log::warn!("Dev MCP: the server is now {url:?}, off this computer: every tool call is refused from now on");
            }
            SERVER_LOCAL.store(local, Ordering::SeqCst);
            *last = Some(url.clone());
        }
    });
}

/// Write the endpoint file whole (a side file renamed onto the name).
fn write_endpoint_file(body: &serde_json::Value) {
    let _ = std::fs::create_dir_all("debug");
    let tmp = format!("{ENDPOINT_FILE}.tmp");
    if std::fs::write(&tmp, serde_json::to_string_pretty(body).unwrap_or_default()).is_ok() {
        let _ = std::fs::rename(&tmp, ENDPOINT_FILE);
    }
}

fn new_token() -> Result<String, String> {
    let mut b = [0u8; 32];
    getrandom::getrandom(&mut b).map_err(|e| format!("no random bytes for the token: {e}"))?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

fn start(state: &EngineState) {
    let portable = crate::storage::mode() == crate::storage::StorageMode::Portable;
    let switch = std::env::var(ENV_SWITCH).ok();
    match decide(switch.as_deref(), portable, &state.gui_state.server_url) {
        Start::Off => {}
        Start::Refused(why) => {
            log::warn!("Dev MCP: {ENV_SWITCH} is set but the endpoint is NOT started: {why}");
            write_endpoint_file(&serde_json::json!({ "ok": false, "refused": why, "pid": std::process::id() }));
        }
        Start::Go => match launch() {
            Ok(url) => log::info!("Dev MCP: serving {url} (docs/ai/mcp.md); endpoint file {ENDPOINT_FILE}"),
            Err(why) => {
                log::warn!("Dev MCP: could not start: {why}");
                write_endpoint_file(&serde_json::json!({ "ok": false, "refused": why, "pid": std::process::id() }));
            }
        },
    }
}

fn launch() -> Result<String, String> {
    let listener = http::bind_loopback(std::net::SocketAddr::from(([127, 0, 0, 1], 0)))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let token = new_token()?;
    let game_dir = std::env::current_dir().map_err(|e| format!("no working folder: {e}"))?;
    let logs_dir = crate::storage::portable_logs_dir();
    let link: Arc<dyn link::GameLink> = Arc::new(link::LiveLink::new(game_dir.clone(), logs_dir.clone()));
    let host: Arc<dyn protocol::ToolHost + Send + Sync> = Arc::new(tools::GameTools::new(link));
    // Before the listener serves: a call must find the funnel's door open.
    ACTIVE.store(true, Ordering::SeqCst);
    http::serve(listener, token.clone(), host).map_err(|e| format!("could not start the listener thread: {e}"))?;
    let url = format!("http://127.0.0.1:{port}{}", http::MCP_PATH);
    let started_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    write_endpoint_file(&serde_json::json!({
        "ok": true,
        "url": url,
        "port": port,
        "token": token,
        "pid": std::process::id(),
        "version": env!("CARGO_PKG_VERSION"),
        "game_dir": game_dir.display().to_string(),
        "run_log": logs_dir.map(|d| d.join("run.log").display().to_string()),
        "protocol_versions": protocol::supported_versions(),
        "started_ms": started_ms,
    }));
    Ok(url)
}
