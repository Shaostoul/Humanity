//! Host a node: run a HumanityOS relay on THIS machine, from inside the app.
//!
//! Hosting used to be shell-only (`HumanityOS --headless`, or a systemd unit on
//! a VPS), which broke the GUI-first rule in CLAUDE.md: "anything an operator /
//! admin / user can configure or do MUST be reachable from inside the app, not
//! only from a shell". This module is that surface. It is rendered as a section
//! of the Relays page (`relay_control.rs`), which is already the one place the
//! operator manages every relay they own.
//!
//! ## How it works
//!
//! There is only ONE binary. The `native` Cargo feature already includes
//! `relay`, so the desktop app links the whole server, and
//! `crate::relay::run_relay()` is exactly what `--headless` calls. The separate
//! relay-only build exists purely so a headless Linux box does not need the
//! GPU/audio/windowing system libraries the desktop build links against; it is
//! not a different server.
//!
//! So "start a node" means: set the config env vars the relay reads (`PORT`,
//! `DATABASE_PATH`, `SERVER_NAME`, `BIND_ADDRESS`), then run `run_relay()` on
//! its own thread with its own tokio runtime so the UI never blocks. "Stop"
//! resolves a oneshot that wins a `select!` against the serve future, which
//! drops the listener and frees the port.
//!
//! ## Honest state, not optimistic state
//!
//! `run_relay()` panics rather than returning errors on the common failures
//! (port taken, unwritable database directory). Two layers keep that from
//! showing up as a silently dead node:
//!
//! 1. Pre-flight: the port is test-bound and the database folder is created
//!    BEFORE the thread spawns, so the usual failure is reported as a sentence
//!    the user can act on instead of a panic in a detached thread.
//! 2. The thread body is wrapped in `catch_unwind`, and the node is only
//!    reported as Running once `GET /health` on the loopback address actually
//!    answers. Started-but-never-answered is a failure, not a success.

use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use egui::{Frame, RichText, Rounding, Stroke};

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::{ChatServer, GuiPage, GuiState};

/// How long we wait for a freshly started node to answer on /health before
/// calling it a failure. Generous: the first boot opens (or creates) the
/// SQLite database, runs migrations, and seeds default channels.
const READY_TIMEOUT: Duration = Duration::from_secs(20);
/// Gap between /health polls while waiting for readiness.
const READY_POLL: Duration = Duration::from_millis(500);

/// Lifecycle of the node hosted by this app instance.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NodeStatus {
    /// Nothing running; the form is editable.
    Stopped,
    /// The thread is up but /health has not answered yet.
    Starting,
    /// /health answered. People can connect.
    Running,
    /// Stop was requested; waiting for the serve future to unwind.
    Stopping,
    /// It could not start, or it exited on its own. `message` says why.
    Failed,
}

/// Something a worker thread has to tell the UI.
enum NodeEvent {
    /// /health answered; the node is genuinely serving.
    Ready,
    /// It never came up (or the relay panicked on the way).
    Failed(String),
    /// The serve future returned. Expected after Stop, a fault otherwise.
    Exited(String),
    /// This node's own federation public key, fetched from
    /// /api/server-info once the node is up. Shown with a copy button so
    /// the operator of ANOTHER server can register this NAT'd home node
    /// with /server-add-key without this node needing any public URL.
    ServerKey(String),
}

/// The one node this process hosts.
///
/// A process can only bind a given port once and only one `run_relay()` makes
/// sense per app instance, so this is deliberately a singleton rather than a
/// field on `GuiState`: the state IS process-global, and modelling it that way
/// keeps the whole feature in one file.
struct LocalNode {
    status: NodeStatus,
    /// User-facing status/error line. Empty when there is nothing to say.
    message: String,

    // Editable before start.
    port_input: String,
    db_input: String,
    name_input: String,
    /// "Only this computer": listen on 127.0.0.1 instead of every interface
    /// (the relay setting BIND_ADDRESS). Off by default, because hosting is
    /// usually for other people on the same network.
    local_only: bool,

    // Frozen at start, so the running node is described by what it ACTUALLY
    // uses rather than by whatever the form says right now.
    running_port: u16,
    running_db: String,
    running_name: String,
    running_local_only: bool,
    started_at: Option<Instant>,
    /// This machine's address on the local network, resolved once per start.
    lan_ip: Option<String>,
    /// This node's federation public key (from /api/server-info after
    /// Ready). Empty until fetched. The identity another relay's operator
    /// pins with /server-add-key to federate with this node.
    server_pubkey: String,
    /// Open folder picker for the database location, if any.
    db_picker: Option<crate::gui::widgets::file_browser::FilePickerState>,

    stop_tx: Option<tokio::sync::oneshot::Sender<()>>,
    events: Option<Receiver<NodeEvent>>,
}

impl LocalNode {
    fn new() -> Self {
        Self {
            status: NodeStatus::Stopped,
            message: String::new(),
            port_input: "3210".to_string(),
            db_input: default_db_path(),
            name_input: default_server_name(),
            local_only: false,
            running_port: 0,
            running_db: String::new(),
            running_name: String::new(),
            running_local_only: false,
            started_at: None,
            lan_ip: None,
            server_pubkey: String::new(),
            db_picker: None,
            stop_tx: None,
            events: None,
        }
    }
}

static NODE: OnceLock<Mutex<LocalNode>> = OnceLock::new();

/// The app identity's public key, noted each frame by draw_section (and by
/// autostart) so `start` can grant the node's OWNER the admin role in its
/// database without threading GuiState into the start path.
static OWNER_KEY: Mutex<String> = Mutex::new(String::new());

pub(crate) fn note_owner_key(key: &str) {
    if let Ok(mut g) = OWNER_KEY.lock() {
        if *g != key {
            *g = key.to_string();
        }
    }
}

fn owner_key_for_admin() -> Option<String> {
    OWNER_KEY
        .lock()
        .ok()
        .map(|g| g.clone())
        .filter(|s| !s.trim().is_empty())
}

fn node() -> &'static Mutex<LocalNode> {
    NODE.get_or_init(|| Mutex::new(LocalNode::new()))
}

/// Boot-time autostart: reproduce the node the operator last had running
/// (port/db/name saved when Start was pressed; disarmed by Stop). Called
/// once from engine init, right after the config lands in GuiState -- the
/// field-test failure this fixes is "my self-relay is unreachable after
/// every app restart" (the node simply was not running).
pub fn autostart_if_configured(state: &crate::gui::GuiState) {
    note_owner_key(&state.profile_public_key);
    if !state.host_node_autostart {
        return;
    }
    let Ok(mut n) = node().lock() else { return };
    if matches!(n.status, NodeStatus::Running | NodeStatus::Starting) {
        return;
    }
    if !state.host_node_port.trim().is_empty() {
        n.port_input = state.host_node_port.clone();
    }
    if !state.host_node_db.trim().is_empty() {
        n.db_input = state.host_node_db.clone();
    }
    if !state.host_node_name.trim().is_empty() {
        n.name_input = state.host_node_name.clone();
    }
    n.local_only = state.host_node_local_only;
    log::info!(
        "Host node autostart: starting the local relay on port {}",
        n.port_input
    );
    start(&mut n);
}

/// The loopback URL of the node this app hosts, when it is up.
///
/// The Relays page calls this so a running local node shows in the "MY RELAYS"
/// rail alongside the remote ones, instead of being invisible until you scroll.
pub fn running_local_url() -> Option<String> {
    let n = node().lock().ok()?;
    if n.status == NodeStatus::Running {
        Some(format!("http://127.0.0.1:{}", n.running_port))
    } else {
        None
    }
}

/// Display name for the rail row of the locally hosted node.
pub fn running_local_name() -> String {
    node()
        .lock()
        .ok()
        .map(|n| {
            if n.running_name.trim().is_empty() {
                "This PC".to_string()
            } else {
                format!("{} (this PC)", n.running_name.trim())
            }
        })
        .unwrap_or_else(|| "This PC".to_string())
}

// ───────────────────────────── defaults ─────────────────────────────

/// Where a node's database goes by default: beside the app's own config, in a
/// `relay/` folder. Uses `AppConfig::config_path()` so it follows portable
/// mode and `HUMANITY_DATA_DIR` instead of hardcoding %APPDATA%.
///
/// Deliberately NOT `data/relay.db` (the repo-relative path the VPS uses): a
/// desktop app's working directory is wherever the exe happens to live, and a
/// database written there would be lost the moment the user moved the folder.
fn default_db_path() -> String {
    let dir = crate::config::AppConfig::config_path()
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    dir.join("relay").join("relay.db").display().to_string()
}

/// A friendly default server name. The machine name is what a person would
/// call this box, so it is a better first guess than a generic string.
fn default_server_name() -> String {
    let host = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_default();
    let host = host.trim();
    if host.is_empty() {
        "My HumanityOS node".to_string()
    } else {
        format!("{host}'s node")
    }
}

/// This machine's address on the local network.
///
/// Opens a UDP socket and points it at a public address. No packet is ever
/// sent: `connect` on UDP only fixes the default peer, which makes the OS
/// choose the outbound interface, so `local_addr` then reports the real LAN
/// address instead of 0.0.0.0. Returns None on a machine with no route out,
/// where there is no LAN address worth showing.
fn lan_address() -> Option<String> {
    let sock = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;
    let ip = sock.local_addr().ok()?.ip();
    if ip.is_loopback() || ip.is_unspecified() {
        None
    } else {
        Some(ip.to_string())
    }
}

/// The address the node listens on: loopback for "Only this computer",
/// otherwise every interface (what other devices on the network need). Passed
/// to the relay as its BIND_ADDRESS setting.
fn node_bind_ip(local_only: bool) -> std::net::IpAddr {
    if local_only {
        std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
    } else {
        crate::relay::DEFAULT_BIND_ADDRESS
    }
}

/// The addresses Start test-binds before it starts anything: the address the
/// relay will listen on, and loopback as well when that is not already it.
///
/// Loopback is checked for a network node because on Windows a program
/// holding only 127.0.0.1:PORT does NOT block a 0.0.0.0:PORT bind: checking
/// the wildcard alone would let the node start and then quietly lose every
/// local connection to the squatter, because the more specific bind wins.
///
/// A node for this computer only is checked on loopback ONLY. Test-binding
/// 0.0.0.0 is itself a listen on every interface, which on Windows raises the
/// firewall prompt; a node that never needs the network must not cause one.
fn preflight_addrs(bind_ip: std::net::IpAddr, port: u16) -> Vec<std::net::SocketAddr> {
    let mut v = vec![std::net::SocketAddr::new(bind_ip, port)];
    if !bind_ip.is_loopback() {
        v.push(std::net::SocketAddr::from(([127, 0, 0, 1], port)));
    }
    v
}

/// Test-bind each address in turn and release it at once. The first one that
/// cannot be bound, with the reason; None when every one is free.
fn first_busy(addrs: &[std::net::SocketAddr]) -> Option<(std::net::SocketAddr, std::io::Error)> {
    for &addr in addrs {
        match std::net::TcpListener::bind(addr) {
            Ok(l) => drop(l),
            Err(e) => return Some((addr, e)),
        }
    }
    None
}

/// Best-effort text of a caught panic payload.
fn panic_text(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "the server thread stopped unexpectedly".to_string()
    }
}

/// "5m 12s" / "2h 7m" style uptime.
fn fmt_uptime(d: Duration) -> String {
    let secs = d.as_secs();
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m {s}s")
    } else {
        format!("{s}s")
    }
}

// ───────────────────────────── lifecycle ─────────────────────────────

/// Validate the form, then bring the node up on its own thread.
///
/// Everything that can be checked cheaply is checked BEFORE the thread exists,
/// because a failure inside `run_relay()` is a panic on a detached thread, and
/// "nothing happened" is the worst possible feedback.
fn start(n: &mut LocalNode) {
    let port: u16 = match n.port_input.trim().parse::<u32>() {
        Ok(p) if (1..=65535).contains(&p) => p as u16,
        _ => {
            n.status = NodeStatus::Failed;
            n.message = "Port must be a whole number between 1 and 65535. 3210 is the usual one."
                .to_string();
            return;
        }
    };

    let db = n.db_input.trim().to_string();
    if db.is_empty() {
        n.status = NodeStatus::Failed;
        n.message = "Pick a file for the database, for example a relay.db inside a folder you own."
            .to_string();
        return;
    }

    // The common failure by far: something already holds the port. Test-bind and
    // release it here so the message names the real problem. Which addresses,
    // and why loopback is among them even for a network node: preflight_addrs.
    // Unit tests hold a loopback port and assert it is reported.
    //
    // There is a small window between this check and the relay's own bind in
    // which another program could take the port; catch_unwind below is the net
    // for that rarer case.
    let bind_ip = node_bind_ip(n.local_only);
    if let Some((_, e)) = first_busy(&preflight_addrs(bind_ip, port)) {
        n.status = NodeStatus::Failed;
        n.message = format!(
            "Port {port} is not available ({e}). Another program is already using it, \
             possibly a node you started earlier. Try a different port."
        );
        return;
    }

    // Only once everything cheap has passed do we touch the disk. The relay
    // creates this directory itself, but it PANICS if it cannot, so do it here
    // where the failure can be a sentence instead. Doing it last also means a
    // rejected start leaves no folders behind.
    let db_path = std::path::PathBuf::from(&db);
    if let Some(parent) = db_path.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                n.status = NodeStatus::Failed;
                n.message = format!(
                    "Cannot create the database folder {}: {e}. Pick a location you can write to.",
                    parent.display()
                );
                return;
            }
        }
    }

    // The relay reads its configuration from the environment, the same knobs
    // the VPS systemd unit sets. Setting them here is what makes the in-app
    // node and the headless one the same server with the same behaviour.
    let name = n.name_input.trim().to_string();
    std::env::set_var("PORT", port.to_string());
    std::env::set_var("DATABASE_PATH", &db);
    // Always set, never inherited: the choice on this page decides who can
    // connect, not whatever BIND_ADDRESS the app happened to be launched with.
    std::env::set_var(crate::relay::BIND_ADDRESS_ENV, bind_ip.to_string());
    // The person hosting the node owns it: their identity gets the admin
    // role in the node's database at startup (operator field test 3: the
    // self-relay's Server Settings showed only the USER section, because
    // nothing had ever granted the owner admin).
    match owner_key_for_admin() {
        Some(k) => std::env::set_var("HUMANITY_OWNER_ADMIN_KEY", k),
        None => std::env::remove_var("HUMANITY_OWNER_ADMIN_KEY"),
    }
    if !name.is_empty() {
        std::env::set_var("SERVER_NAME", &name);
    }

    let (ev_tx, ev_rx): (Sender<NodeEvent>, Receiver<NodeEvent>) = std::sync::mpsc::channel();
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();

    // ── The server thread ──
    let relay_tx = ev_tx.clone();
    let spawned = std::thread::Builder::new()
        .name("humanity-local-node".to_string())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = relay_tx
                        .send(NodeEvent::Failed(format!("Could not start the server runtime: {e}")));
                    return;
                }
            };
            // AssertUnwindSafe: on a panic we tear the whole runtime down and
            // report the node as failed, so no partially-updated state is
            // observed afterwards.
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                rt.block_on(async move {
                    tokio::select! {
                        // Serving ended by itself (a fault: it normally runs forever).
                        _ = crate::relay::run_relay() => false,
                        // Stop was pressed. Dropping the serve future closes the
                        // listener and releases the port.
                        _ = stop_rx => true,
                    }
                })
            }));
            // Bounded so a wedged background task cannot hang the app on exit.
            rt.shutdown_timeout(Duration::from_secs(3));
            let _ = match outcome {
                Ok(true) => relay_tx.send(NodeEvent::Exited("Stopped.".to_string())),
                Ok(false) => relay_tx.send(NodeEvent::Exited(
                    "The node stopped on its own. The app log has the details.".to_string(),
                )),
                Err(p) => relay_tx.send(NodeEvent::Failed(format!(
                    "The node could not run: {}. The app log and crash log have the details.",
                    panic_text(&p)
                ))),
            };
        });

    if let Err(e) = spawned {
        n.status = NodeStatus::Failed;
        n.message = format!("Could not start a thread for the node: {e}");
        return;
    }

    // ── The readiness watcher ──
    // Serving is only real once /health answers. Polling loopback avoids
    // claiming success while the database is still migrating (or while the
    // relay is on its way to a panic).
    let health_tx = ev_tx;
    let _ = std::thread::Builder::new()
        .name("humanity-local-node-health".to_string())
        .spawn(move || {
            let url = format!("http://127.0.0.1:{port}/health");
            let deadline = Instant::now() + READY_TIMEOUT;
            while Instant::now() < deadline {
                std::thread::sleep(READY_POLL);
                if ureq::get(&url)
                    .timeout(Duration::from_millis(1000))
                    .call()
                    .is_ok()
                {
                    let _ = health_tx.send(NodeEvent::Ready);
                    // Grab this node's federation public key while we are
                    // already on a background thread: it is what another
                    // relay's operator pins with /server-add-key, so the
                    // page shows it copyable the moment the node is up.
                    if let Ok(resp) = ureq::get(&format!(
                        "http://127.0.0.1:{port}/api/server-info"
                    ))
                    .timeout(Duration::from_secs(3))
                    .call()
                    {
                        if let Ok(body) = resp.into_string() {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                                if let Some(k) = v.get("public_key").and_then(|k| k.as_str()) {
                                    let _ = health_tx.send(NodeEvent::ServerKey(k.to_string()));
                                }
                            }
                        }
                    }
                    return;
                }
            }
            let _ = health_tx.send(NodeEvent::Failed(format!(
                "The node did not answer on port {port} within {} seconds. \
                 The app log has the details.",
                READY_TIMEOUT.as_secs()
            )));
        });

    n.status = NodeStatus::Starting;
    n.message = "Starting the node...".to_string();
    n.running_port = port;
    n.running_db = db;
    n.running_name = name;
    n.running_local_only = n.local_only;
    n.started_at = None;
    // A node for this computer only has no network address to hand out.
    n.lan_ip = if n.local_only { None } else { lan_address() };
    n.stop_tx = Some(stop_tx);
    n.events = Some(ev_rx);
}

/// Ask the node to stop. The thread reports back with an `Exited` event.
fn stop(n: &mut LocalNode) {
    if let Some(tx) = n.stop_tx.take() {
        let _ = tx.send(());
    }
    n.status = NodeStatus::Stopping;
    n.message = "Stopping the node...".to_string();
}

/// Apply one worker event. Events that do not match the current phase are
/// dropped: after Stop was pressed, a late readiness timeout from the watcher
/// thread must not turn a clean stop into a reported failure.
fn apply(n: &mut LocalNode, ev: NodeEvent) {
    match ev {
        NodeEvent::Ready => {
            if n.status == NodeStatus::Starting {
                n.status = NodeStatus::Running;
                n.message.clear();
                n.started_at = Some(Instant::now());
            }
        }
        NodeEvent::ServerKey(k) => {
            n.server_pubkey = k;
        }
        NodeEvent::Failed(m) => {
            if matches!(n.status, NodeStatus::Starting | NodeStatus::Running) {
                n.status = NodeStatus::Failed;
                n.message = m;
            }
        }
        NodeEvent::Exited(m) => {
            if n.status == NodeStatus::Stopping {
                n.status = NodeStatus::Stopped;
                n.message = m;
            } else {
                n.status = NodeStatus::Failed;
                n.message = m;
            }
        }
    }
}

/// Non-blocking drain of the worker channel, once per frame.
fn drain(n: &mut LocalNode) {
    let Some(rx) = n.events.take() else {
        return;
    };
    let mut pending = Vec::new();
    loop {
        match rx.try_recv() {
            Ok(ev) => pending.push(ev),
            Err(TryRecvError::Empty) => break,
            // Both senders dropped without a terminal event: the threads are
            // gone, so treat it as an exit rather than waiting forever.
            Err(TryRecvError::Disconnected) => {
                if matches!(n.status, NodeStatus::Starting | NodeStatus::Running) {
                    pending.push(NodeEvent::Exited(
                        "The node stopped unexpectedly. The app log has the details.".to_string(),
                    ));
                }
                break;
            }
        }
    }
    n.events = Some(rx);
    for ev in pending {
        apply(n, ev);
    }
    if matches!(n.status, NodeStatus::Stopped | NodeStatus::Failed) {
        release(n);
    }
}

/// Let go of a node that is no longer serving.
///
/// The stop signal is fired FIRST, and that is the whole point: a readiness
/// timeout reports Failed while the server thread may still be alive and on its
/// way up. Dropping the handle without signalling would leave a node holding the
/// port that the UI believes does not exist, unstoppable short of quitting the
/// app. On a node that already exited the send simply fails and is ignored.
fn release(n: &mut LocalNode) {
    if let Some(tx) = n.stop_tx.take() {
        let _ = tx.send(());
    }
    n.events = None;
    n.started_at = None;
}

// ───────────────────────────── the surface ─────────────────────────────

/// Small filled dot, matching the rail dots on the Relays page.
fn dot(ui: &mut egui::Ui, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(10.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 5.0, color);
}

/// Draw the "Host a node" section. Rendered inside the Relays page's scroll.
pub fn draw_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    note_owner_key(&state.profile_public_key);
    let mut n = match node().lock() {
        Ok(n) => n,
        // A poisoned lock means a previous panic happened while state was held.
        // Nothing here is worth crashing the app over; say so and move on.
        Err(_) => {
            ui.label(
                RichText::new("Hosting is unavailable in this session (internal state was lost).")
                    .size(theme.font_size_small)
                    .color(theme.danger()),
            );
            return;
        }
    };
    drain(&mut n);

    // Keep the clock and the status honest while anything is in motion.
    if matches!(n.status, NodeStatus::Starting | NodeStatus::Stopping | NodeStatus::Running) {
        ui.ctx().request_repaint_after(Duration::from_millis(400));
    }

    widgets::body_hint(
        ui,
        theme,
        "Run a server on this computer so other people can connect to you directly, \
         without renting anything. It is the same server a hosted relay runs; starting \
         it here just saves you a terminal. It serves for as long as HumanityOS is open.",
    );
    ui.add_space(theme.spacing_sm);

    // ── Status line ──
    let (label, color) = match n.status {
        NodeStatus::Stopped => ("Not running", theme.text_muted()),
        NodeStatus::Starting => ("Starting", theme.warning()),
        NodeStatus::Running => ("Running", theme.success()),
        NodeStatus::Stopping => ("Stopping", theme.warning()),
        NodeStatus::Failed => ("Not running", theme.danger()),
    };
    ui.horizontal(|ui| {
        dot(ui, color);
        ui.label(RichText::new(label).size(theme.font_size_body).color(color));
        if n.status == NodeStatus::Running {
            if let Some(t) = n.started_at {
                ui.label(
                    RichText::new(format!("up {}", fmt_uptime(t.elapsed())))
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            }
        }
    });

    if !n.message.is_empty() {
        ui.add_space(theme.spacing_xs);
        let msg_color = if n.status == NodeStatus::Failed {
            theme.danger()
        } else {
            theme.text_muted()
        };
        ui.label(
            RichText::new(n.message.clone())
                .size(theme.font_size_small)
                .color(msg_color),
        );
    }
    ui.add_space(theme.spacing_md);

    match n.status {
        NodeStatus::Running | NodeStatus::Stopping => draw_running(ui, theme, state, &mut n),
        _ => draw_setup(ui, theme, state, &mut n),
    }
}

/// The editable form, shown whenever nothing is serving.
fn draw_setup(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, n: &mut LocalNode) {
    let starting = n.status == NodeStatus::Starting;

    Frame::none()
        .fill(theme.bg_card())
        .stroke(Stroke::new(1.0, theme.border()))
        .rounding(Rounding::same(theme.border_radius as u8))
        .inner_margin(theme.spacing_md)
        .show(ui, |ui| {
            // form_row is the app-wide label/control pair, so these line up with
            // the Settings and Server Settings forms instead of inventing a
            // second alignment. The help sentence sits under the input, in the
            // control column, where there is room for a full sentence.
            let field_w = (ui.available_width() - 170.0).max(180.0);
            let field = |ui: &mut egui::Ui, label: &str, help: &str, value: &mut String, hint: &str| {
                widgets::form_row(ui, theme, label, |ui| {
                    ui.vertical(|ui| {
                        ui.add_enabled(
                            !starting,
                            egui::TextEdit::singleline(value)
                                .desired_width(field_w)
                                .hint_text(hint),
                        );
                        ui.label(
                            RichText::new(help)
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                        );
                    });
                });
            };

            field(
                ui,
                "Server name",
                "What people see in their server list when they connect.",
                &mut n.name_input,
                "My HumanityOS node",
            );
            field(
                ui,
                "Port",
                "The number people include in the address. 3210 unless it is taken.",
                &mut n.port_input,
                "3210",
            );
            // Who can connect: the relay's BIND_ADDRESS setting, in plain
            // words. The network choice is the default and what hosting for
            // friends needs; "this computer" listens on 127.0.0.1 only.
            widgets::form_row(ui, theme, "Who can connect", |ui| {
                ui.vertical(|ui| {
                    ui.set_max_width(field_w);
                    ui.add_enabled_ui(!starting, |ui| {
                        ui.radio_value(&mut n.local_only, false, "Devices on my network");
                        ui.radio_value(&mut n.local_only, true, "Only this computer");
                    });
                    ui.label(
                        RichText::new(if n.local_only {
                            "Nothing else can reach this node, so Windows will not ask about \
                             the firewall. Good for trying a node out on your own."
                        } else {
                            "Anyone on the same network can connect. The first time, Windows \
                             may ask whether to let HumanityOS through its firewall: allow it \
                             on private networks, or other devices will not get in."
                        })
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                    );
                });
            });
            field(
                ui,
                "Database file",
                "Where this node keeps its messages, channels, and members. Back this file up.",
                &mut n.db_input,
                "relay.db",
            );
            // Folder picker so nobody types a path by hand (operator ask,
            // 2026-08-14). Picks the FOLDER; the file keeps its name (or
            // gets the default relay.db when the field was empty/a dir).
            // In the control column under the Database file field, with the
            // note wrapping at the field's width. As a plain label beside the
            // button in a `ui.horizontal` it could not wrap, and it pushed the
            // whole form card past the page edge (2026-09-27 snapshot review).
            widgets::form_row(ui, theme, "", |ui| {
                ui.vertical(|ui| {
                    ui.set_max_width(field_w);
                    if widgets::Button::secondary("Browse for folder").show(ui, theme) {
                        let start = std::path::Path::new(n.db_input.trim())
                            .parent()
                            .filter(|p| p.is_dir())
                            .map(|p| p.to_path_buf());
                        n.db_picker = Some(
                            crate::gui::widgets::file_browser::FilePickerState::new_dir_picker(start),
                        );
                    }
                    ui.label(
                        RichText::new(
                            "The default lives in your Windows user profile (AppData), which \
                             survives app updates and needs no admin rights. Pick any folder \
                             you prefer; a drive you back up is a fine choice.",
                        )
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                    );
                });
            });

            ui.add_space(theme.spacing_xs);
            // Autostart: the field-test failure was simply "the node is not
            // running after an app restart". Starting the node turns this on
            // (Stop turns it off); the checkbox is the manual override.
            let mut auto = state.host_node_autostart;
            if ui
                .checkbox(&mut auto, "Start this node automatically when the app opens")
                .on_hover_text(
                    "Reproduces this exact node (port, database, name, who can connect) at every launch, \
                     so your self-hosted server is reachable again without visiting this page.",
                )
                .changed()
            {
                state.host_node_autostart = auto;
                crate::config::AppConfig::from_gui_state(state).save();
            }
            ui.add_space(theme.spacing_xs);
            ui.horizontal(|ui| {
                if widgets::Button::primary(if starting { "Starting..." } else { "Start node" })
                    .disabled(starting)
                    .tooltip("Runs the HumanityOS server on this computer, the same one a hosted relay runs.")
                    .show(ui, theme)
                {
                    start(n);
                    if n.status != NodeStatus::Failed {
                        // Remember the running shape + arm autostart so an
                        // app restart brings this server back by itself.
                        state.host_node_autostart = true;
                        state.host_node_port = n.port_input.clone();
                        state.host_node_db = n.db_input.clone();
                        state.host_node_name = n.name_input.clone();
                        state.host_node_local_only = n.local_only;
                        crate::config::AppConfig::from_gui_state(state).save();
                    }
                }
                if widgets::Button::ghost("Reset to defaults")
                    .disabled(starting)
                    .show(ui, theme)
                {
                    n.port_input = "3210".to_string();
                    n.db_input = default_db_path();
                    n.name_input = default_server_name();
                    n.local_only = false;
                    n.message.clear();
                    n.status = NodeStatus::Stopped;
                }
            });

            // Folder picker modal (take/put dance keeps the borrow simple).
            if let Some(mut picker) = n.db_picker.take() {
                use crate::gui::widgets::file_browser::{file_picker_modal, FilePickerResult};
                match file_picker_modal(
                    ui.ctx(),
                    theme,
                    &mut picker,
                    "Choose a folder for the node database",
                ) {
                    FilePickerResult::Open => n.db_picker = Some(picker),
                    FilePickerResult::Cancelled => {}
                    FilePickerResult::Picked(dir) => {
                        // Keep the existing file name when there is one;
                        // otherwise the standard relay.db.
                        let file = std::path::Path::new(n.db_input.trim())
                            .file_name()
                            .and_then(|f| f.to_str())
                            .filter(|f| f.ends_with(".db"))
                            .unwrap_or("relay.db")
                            .to_string();
                        n.db_input = dir.join(file).display().to_string();
                    }
                }
            }
        });
}

/// What a serving node looks like: the addresses to hand out, and Stop.
fn draw_running(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, n: &mut LocalNode) {
    let local_url = format!("http://127.0.0.1:{}", n.running_port);
    let lan_url = n
        .lan_ip
        .as_ref()
        .map(|ip| format!("http://{ip}:{}", n.running_port));

    Frame::none()
        .fill(theme.bg_card())
        .stroke(Stroke::new(1.0, theme.border()))
        .rounding(Rounding::same(theme.border_radius as u8))
        .inner_margin(theme.spacing_md)
        .show(ui, |ui| {
            let row = |ui: &mut egui::Ui, label: &str, value: &str, copyable: bool| {
                widgets::form_row(ui, theme, label, |ui| {
                    ui.label(
                        RichText::new(value)
                            .size(theme.font_size_body)
                            .color(theme.text_primary()),
                    );
                    if copyable && widgets::Button::ghost("Copy").show(ui, theme) {
                        ui.ctx().copy_text(value.to_string());
                    }
                });
            };

            if !n.running_name.is_empty() {
                row(ui, "Server name", &n.running_name.clone(), false);
            }
            row(ui, "On this computer", &local_url, true);
            match &lan_url {
                _ if n.running_local_only => row(
                    ui,
                    "On your network",
                    "not shared: this node accepts this computer only",
                    false,
                ),
                Some(u) => row(ui, "On your network", u, true),
                None => row(
                    ui,
                    "On your network",
                    "no local network address found",
                    false,
                ),
            }
            row(ui, "Database file", &n.running_db.clone(), true);
            if !n.server_pubkey.is_empty() {
                // Shown truncated (the full key is 64 hex chars); Copy
                // carries the whole thing. This is the identity another
                // relay's admin pins with /server-add-key to federate with
                // this node, and it needs NO port forward or public URL:
                // this node dials OUT and the peering rides that socket.
                let short = format!(
                    "{}\u{2026}{}",
                    &n.server_pubkey[..n.server_pubkey.len().min(12)],
                    &n.server_pubkey[n.server_pubkey.len().saturating_sub(6)..]
                );
                widgets::form_row(ui, theme, "Federation key", |ui| {
                    ui.label(
                        RichText::new(short)
                            .size(theme.font_size_body)
                            .monospace()
                            .color(theme.text_primary()),
                    );
                    if widgets::Button::ghost("Copy").show(ui, theme) {
                        ui.ctx().copy_text(n.server_pubkey.clone());
                    }
                });
            }
        });

    ui.add_space(theme.spacing_sm);
    // Who can reach it depends on how it was started ("Who can connect").
    let reach = if n.running_local_only {
        "This node was started for this computer only, so no other device can reach it. \
         To share it, stop it, choose \"Devices on my network\", and start it again."
    } else {
        "Anyone on the same network (same house, same wifi) can connect using the network \
         address. Reaching it from outside your home also needs a port forward on your \
         router, which no app can set up for you."
    };
    ui.label(
        RichText::new(format!(
            "{reach} FEDERATING is different: to pair this node with another server, no \
             port forward is needed at all. Copy the Federation key above, and on the other \
             server run: /server-add-key <key> <name>, then /server-trust <key> 2. This node \
             then connects OUT to that server and the partnership rides that connection."
        ))
        .size(theme.font_size_small)
        .color(theme.text_muted()),
    );
    ui.add_space(theme.spacing_md);

    let stopping = n.status == NodeStatus::Stopping;
    let share_url = lan_url.clone().unwrap_or_else(|| local_url.clone());
    ui.horizontal(|ui| {
        if widgets::Button::danger(if stopping { "Stopping..." } else { "Stop node" })
            .disabled(stopping)
            .tooltip("Shuts the server down and frees the port. Nothing is deleted. \
                      Also turns off launch autostart until you start it again.")
            .show(ui, theme)
        {
            stop(n);
            // An explicit Stop means "I do not want this running": disarm
            // autostart too, or the next launch would resurrect it.
            state.host_node_autostart = false;
            crate::config::AppConfig::from_gui_state(state).save();
        }
        if widgets::Button::secondary("Add to my servers")
            .disabled(stopping)
            .tooltip("Puts this node in your Chat server list so you can connect to it like any other.")
            .show(ui, theme)
        {
            let url = local_url.clone();
            if !state.chat_servers.iter().any(|s| s.url == url) {
                let name = if n.running_name.trim().is_empty() {
                    "My node".to_string()
                } else {
                    n.running_name.trim().to_string()
                };
                state.chat_servers.push(ChatServer {
                    id: format!("srv_{url}"),
                    name,
                    url,
                    connected: false,
                    channels: Vec::new(),
                    voice_channels: Vec::new(),
                });
                state.settings_dirty = true; // saved servers persist now
            }
            state.active_page = GuiPage::Chat;
        }
        if widgets::Button::ghost("Copy address to share").show(ui, theme) {
            ui.ctx().copy_text(share_url.clone());
        }
    });

    ui.add_space(theme.spacing_sm);
    ui.label(
        RichText::new(
            "To choose what this node offers, connect to it and open Server Settings. \
             The first person to claim it becomes its admin.",
        )
        .size(theme.font_size_small)
        .color(theme.text_muted()),
    );
}

/// Test-only: drive the singleton into a serving state so the snapshot harness
/// can render what a running node looks like without binding a port or opening
/// a database. Compiled out of every real build.
#[cfg(test)]
pub(crate) fn force_running_for_snapshot(port: u16, name: &str, db: &str, lan: Option<&str>) {
    let mut n = node().lock().expect("host node state");
    n.status = NodeStatus::Running;
    n.message.clear();
    n.running_port = port;
    n.running_name = name.to_string();
    n.running_db = db.to_string();
    n.lan_ip = lan.map(|s| s.to_string());
    // checked_sub because Instant has a platform floor (QPC since boot on
    // Windows); shortly after a reboot a plain subtraction would panic.
    let now = Instant::now();
    n.started_at = Some(now.checked_sub(Duration::from_secs(312)).unwrap_or(now));
    // A serving node always has its server key by now (the worker sends
    // NodeEvent::ServerKey during start). Without one the snapshot drew the
    // "Copy the Federation key above" advice with no key row above it, a
    // state no player ever sees. Any 64 hex characters will do.
    n.server_pubkey = "3f9a1c07be52d4e8a6017c93f2b5d8e04a6c19f7b3e2d05c8a41f6e97b2d0c35".to_string();
    n.events = None;
    n.stop_tx = None;
}

/// Test-only: fill the stopped node's setup form with fixed values. The real
/// defaults read this machine's name and profile folder, so without this the
/// setup snapshot showed whichever PC ran the test and could never be a stable
/// baseline. Undo with `reset_for_snapshot`.
#[cfg(test)]
pub(crate) fn seed_setup_for_snapshot(name: &str, db: &str) {
    let mut n = node().lock().expect("host node state");
    *n = LocalNode::new();
    n.name_input = name.to_string();
    n.db_input = db.to_string();
}

/// Test-only: undo `force_running_for_snapshot`. The state is process-global,
/// so a snapshot that leaves it Running would add a "this PC" row to the rail
/// of every later Relays snapshot.
#[cfg(test)]
pub(crate) fn reset_for_snapshot() {
    if let Ok(mut n) = node().lock() {
        *n = LocalNode::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_node_is_stopped_with_usable_defaults() {
        let n = LocalNode::new();
        assert_eq!(n.status, NodeStatus::Stopped);
        assert_eq!(n.port_input, "3210");
        assert!(!n.db_input.is_empty(), "a default database path must be offered");
        assert!(!n.name_input.is_empty(), "a default server name must be offered");
    }

    #[test]
    fn a_bad_port_fails_with_a_message_and_starts_nothing() {
        for bad in ["", "abc", "0", "70000", "-1"] {
            let mut n = LocalNode::new();
            n.port_input = bad.to_string();
            start(&mut n);
            assert_eq!(n.status, NodeStatus::Failed, "port {bad:?} must be rejected");
            assert!(!n.message.is_empty(), "port {bad:?} must explain itself");
            assert!(n.events.is_none(), "port {bad:?} must not spawn a node");
            assert!(n.stop_tx.is_none());
        }
    }

    #[test]
    fn an_empty_database_path_fails_before_spawning() {
        let mut n = LocalNode::new();
        n.db_input = "   ".to_string();
        start(&mut n);
        assert_eq!(n.status, NodeStatus::Failed);
        assert!(n.events.is_none());
    }

    // NO TEST IN THIS FILE LISTENS ON 0.0.0.0, and none may (2026-10-03).
    //
    // The lib test binary lives at target/<profile>/deps/humanity_engine-<hash>.exe,
    // a NEW path after every version bump and in every worktree, and on Windows
    // a program that listens on the wildcard address makes Windows Defender
    // Firewall stop the person at the keyboard with an "allow this app?" prompt,
    // once per path. Two busy-port tests here used to do exactly that (one held
    // 0.0.0.0 itself, the other made start() test-bind it), and their binaries
    // were 209 of the 274 HumanityOS paths in the operator's firewall rule list.
    // So the end-to-end busy-port test runs a node for "Only this computer",
    // which test-binds loopback only, and the network case is pinned by
    // checking WHICH addresses are tested (preflight_addrs) without binding.

    /// A node for this computer only, pointed at a loopback port someone else
    /// holds, must fail loudly and spawn nothing.
    //
    // Red first, 2026-10-03, with the port check made to check nothing:
    // start() went ahead and this failed with "assertion `left == right`
    // failed: a held port was not detected / left: Starting / right: Failed".
    // That red run spawned a relay thread, on 127.0.0.1 because start() hands
    // the relay BIND_ADDRESS, so even a regression here never listens on the
    // wildcard address; its bind of the held port then failed.
    #[test]
    fn a_port_held_on_loopback_is_reported_not_silently_swallowed() {
        let held = std::net::TcpListener::bind(std::net::SocketAddr::from(([127, 0, 0, 1], 0)))
            .expect("bind a scratch loopback port");
        let port = held.local_addr().unwrap().port();

        let mut n = LocalNode::new();
        n.port_input = port.to_string();
        n.local_only = true;
        // A scratch path, so a regression that gets past the port check cannot
        // write a database into the real profile directory during tests.
        n.db_input = std::env::temp_dir()
            .join("humanity-host-node-test")
            .join("relay.db")
            .display()
            .to_string();
        start(&mut n);

        assert_eq!(n.status, NodeStatus::Failed, "a held port was not detected");
        assert!(
            n.message.contains(&port.to_string()),
            "the message must name the port, got: {}",
            n.message
        );
        assert!(n.events.is_none(), "nothing should have been spawned");
        assert!(n.stop_tx.is_none());
        drop(held);
    }

    // Red first, 2026-10-03, with preflight_addrs testing the wildcard alone
    // whatever the mode: "a node for this computer only must never test-bind
    // the wildcard address (it raises the Windows firewall prompt), got
    // [0.0.0.0:3210]". (The list before this change, [0.0.0.0:PORT,
    // 127.0.0.1:PORT] for every node, holds the wildcard too, so it cannot
    // pass either.) Pure: nothing is bound.
    #[test]
    fn preflight_for_this_computer_only_never_touches_the_wildcard_address() {
        let ip = node_bind_ip(true);
        assert!(ip.is_loopback(), "\"Only this computer\" listens on loopback, got {ip}");
        let addrs = preflight_addrs(ip, 3210);
        assert!(
            addrs.iter().all(|a| a.ip().is_loopback()),
            "a node for this computer only must never test-bind the wildcard address \
             (it raises the Windows firewall prompt), got {addrs:?}"
        );
        assert_eq!(addrs, vec![std::net::SocketAddr::from(([127, 0, 0, 1], 3210))]);
    }

    // The Windows case the old loopback-squatter test caught: a 0.0.0.0 bind
    // succeeds while another program holds 127.0.0.1 on the same port, so a
    // network node must test loopback as well as its own wildcard address, or
    // it starts and quietly loses every local connection to the squatter.
    // Pinned without binding the wildcard (see the note above).
    //
    // Red first, 2026-10-03, with preflight_addrs testing the wildcard alone:
    // "a network node must also test loopback (the Windows squatter case), got
    // [0.0.0.0:3210]". Pure: nothing is bound.
    #[test]
    fn preflight_for_the_network_tests_its_own_address_and_loopback() {
        let ip = node_bind_ip(false);
        assert!(ip.is_unspecified(), "\"Devices on my network\" listens everywhere, got {ip}");
        assert_eq!(ip, crate::relay::DEFAULT_BIND_ADDRESS, "the same default as a headless relay");
        let addrs = preflight_addrs(ip, 3210);
        assert!(
            addrs.contains(&std::net::SocketAddr::new(ip, 3210)),
            "a network node must test the address it will listen on, got {addrs:?}"
        );
        assert!(
            addrs.contains(&std::net::SocketAddr::from(([127, 0, 0, 1], 3210))),
            "a network node must also test loopback (the Windows squatter case), got {addrs:?}"
        );
    }

    // Red first, 2026-10-03, with first_busy made to test nothing: "assertion
    // `left == right` failed: the second address was never tested, so a
    // squatter there would be missed / left: None / right:
    // Some(127.0.0.1:49477)". Binds loopback only: port 0
    // (always free) and then the held port.
    #[test]
    fn preflight_tests_every_address_not_only_the_first() {
        let held = std::net::TcpListener::bind(std::net::SocketAddr::from(([127, 0, 0, 1], 0)))
            .expect("bind a scratch loopback port");
        let busy = held.local_addr().unwrap();
        let free = std::net::SocketAddr::from(([127, 0, 0, 1], 0));
        let found = first_busy(&[free, busy]).map(|(a, _)| a);
        assert_eq!(
            found,
            Some(busy),
            "the second address was never tested, so a squatter there would be missed"
        );
        assert!(first_busy(&[free]).is_none(), "a free address is not reported busy");
        drop(held);
    }

    #[test]
    fn a_late_failure_after_stop_does_not_overwrite_a_clean_stop() {
        // The readiness watcher can time out AFTER the user pressed Stop. A
        // clean stop must stay a clean stop.
        let mut n = LocalNode::new();
        n.status = NodeStatus::Stopping;
        apply(&mut n, NodeEvent::Failed("timed out".to_string()));
        assert_eq!(n.status, NodeStatus::Stopping);
        apply(&mut n, NodeEvent::Exited("Stopped.".to_string()));
        assert_eq!(n.status, NodeStatus::Stopped);
    }

    #[test]
    fn serving_is_only_claimed_once_health_answers() {
        let mut n = LocalNode::new();
        n.status = NodeStatus::Starting;
        assert_eq!(n.status, NodeStatus::Starting, "starting is not running");
        apply(&mut n, NodeEvent::Ready);
        assert_eq!(n.status, NodeStatus::Running);
        assert!(n.started_at.is_some(), "uptime starts when serving starts");
    }

    #[test]
    fn a_readiness_timeout_still_tells_the_server_thread_to_stop() {
        // The watcher can give up while the relay is still on its way up. If the
        // stop handle were just dropped, the UI would say "not running" while a
        // server held the port, with no way to stop it but quitting the app.
        let (tx, mut rx) = tokio::sync::oneshot::channel::<()>();
        let mut n = LocalNode::new();
        n.status = NodeStatus::Starting;
        n.stop_tx = Some(tx);

        apply(&mut n, NodeEvent::Failed("did not answer in time".to_string()));
        assert_eq!(n.status, NodeStatus::Failed);
        release(&mut n);

        assert!(n.stop_tx.is_none());
        assert!(
            rx.try_recv().is_ok(),
            "a node reported as failed must be told to stop, or it holds the port forever"
        );
    }

    #[test]
    fn an_unexpected_exit_while_running_is_a_failure() {
        let mut n = LocalNode::new();
        n.status = NodeStatus::Running;
        apply(&mut n, NodeEvent::Exited("it died".to_string()));
        assert_eq!(n.status, NodeStatus::Failed);
        assert!(n.message.contains("it died"));
    }
}
