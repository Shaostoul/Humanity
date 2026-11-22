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

use egui::{Frame, RichText, CornerRadius, Stroke};

use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Hint};
use crate::gui::{ChatServer, GuiPage, GuiState};
use crate::relay::audience::owner_port_for;

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
    /// The Public address field: the names people reach this node by (PUBLIC_HOSTS, C10).
    public_input: String,
    /// "Only this computer": listen on 127.0.0.1 instead of every interface
    /// (the relay setting BIND_ADDRESS). Off by default, because hosting is
    /// usually for other people on the same network.
    local_only: bool,
    /// "Also answer on my local network": with a public address, also list the LAN address (M1 of
    /// the review of the signed requests' round 5, 2026-11-21). Off by default: a forwarder on the
    /// network that adds no header makes its visitors local ([`node_public_hosts`]).
    lan_too: bool,
    /// "This computer is on a Tailscale network" (L3, 2026-11-21): the relay's ON_TAILNET.
    tailnet: bool,
    /// The saved node was due to start at launch but was held back, because a
    /// script launched this copy of the app (see autostart). The first click
    /// into the window starts it; so does the Start button.
    autostart_waiting: bool,

    // Frozen at start, so the running node is described by what it ACTUALLY
    // uses rather than by whatever the form says right now.
    running_port: u16,
    running_db: String,
    running_name: String,
    running_local_only: bool,
    /// The Public address field as the node was started with it: it then lists no loopback name,
    /// and no LAN name unless `running_lan_too` (H1 and M1, 2026-11-21).
    running_public: String,
    running_lan_too: bool,
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
            public_input: String::new(),
            local_only: false,
            lan_too: false,
            tailnet: false,
            autostart_waiting: false,
            running_port: 0,
            running_db: String::new(),
            running_name: String::new(),
            running_local_only: false,
            running_public: String::new(),
            running_lan_too: false,
            started_at: None,
            lan_ip: None,
            server_pubkey: String::new(),
            db_picker: None,
            stop_tx: None,
            events: None,
        }
    }
}

/// What the Host a node section remembers between launches (GuiState `host_node`, saved in
/// AppConfig's `host_node_*` fields): the form as it was when the node last started, and whether it
/// starts by itself at launch.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostNodeSettings {
    pub autostart: bool,
    pub port: String,
    pub db: String,
    pub name: String,
    /// The Public address field.
    pub public: String,
    pub local_only: bool,
    /// "Also answer on my local network" (M1).
    pub lan_too: bool,
    /// "This computer is on a Tailscale network" (L3).
    pub tailnet: bool,
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
/// every app restart" (the node simply was not running). `background`: how
/// this copy was launched (`autostart`); the engine passes its launch policy's
/// answer in (engine/launch_focus.rs `launch_in_background`), because this page
/// is the GUI crate's and the policy is the engine's (crate split rung 0, part B).
pub fn autostart_if_configured(state: &crate::gui::GuiState, background: bool) {
    note_owner_key(&state.profile_public_key);
    let Ok(mut n) = node().lock() else { return };
    autostart(&mut n, state, background);
}

/// The first click into a window a script opened (the same moment that
/// window gets its sound, launch_focus::open_audio_on_click): a person is
/// using this copy of the app after all, so start the saved node autostart
/// held back. Does nothing when nothing was held back.
pub fn autostart_on_first_click(state: &crate::gui::GuiState) {
    note_owner_key(&state.profile_public_key);
    let Ok(mut n) = node().lock() else { return };
    resume_held_autostart(&mut n, state);
}

/// What autostart does with the saved node, given how this copy of the app
/// was launched (`background` is launch_focus::launch_in_background()).
///
/// A copy a SCRIPT launched does not start it, it waits (2026-10-03). Agents
/// boot the app all day (probe rigs, boot checks, `just launch-bg` from a
/// worktree, a fresh versioned archive), each from an exe path Windows has
/// never seen, and they use the operator's real profile. His saved node is a
/// network node (0.0.0.0:3210), so every one of those boots listened on every
/// interface from a new path, and Windows Defender Firewall stopped him with
/// an "allow this app?" prompt for each. It also opened his real node
/// database from an agent's build. Those launches already get no focus and no
/// sound for the same reason: he is using the one computer. A person who
/// opens the app through a script of their own (a launcher, a .bat) gets the
/// node the moment they click into the window, or with Start.
fn autostart(n: &mut LocalNode, state: &GuiState, background: bool) {
    if !state.host_node.autostart {
        return;
    }
    if matches!(n.status, NodeStatus::Running | NodeStatus::Starting) {
        return;
    }
    if !state.host_node.port.trim().is_empty() {
        n.port_input = state.host_node.port.clone();
    }
    if !state.host_node.db.trim().is_empty() {
        n.db_input = state.host_node.db.clone();
    }
    if !state.host_node.name.trim().is_empty() {
        n.name_input = state.host_node.name.clone();
    }
    n.public_input = state.host_node.public.clone();
    n.local_only = state.host_node.local_only;
    n.lan_too = state.host_node.lan_too;
    n.tailnet = state.host_node.tailnet;
    if background {
        n.autostart_waiting = true;
        n.message = "Your saved node has not started yet: a program opened this copy of \
                     HumanityOS, not you. It starts when you click into the window, or \
                     press Start node."
            .to_string();
        log::info!(
            "Host node autostart: held back (this instance was launched in the background by \
             a script); the first click into the window starts it on port {}",
            n.port_input
        );
        return;
    }
    log::info!(
        "Host node autostart: starting the local relay on port {}",
        n.port_input
    );
    start(n);
}

/// Start the node autostart held back, once. See autostart.
fn resume_held_autostart(n: &mut LocalNode, state: &GuiState) {
    if !std::mem::take(&mut n.autostart_waiting) {
        return;
    }
    n.message.clear();
    autostart(n, state, false);
}

/// The address this app reaches the node it hosts by, when it is up: the node's owner's door.
///
/// The Relays page calls this so a running local node shows in the "MY RELAYS"
/// rail alongside the remote ones, instead of being invisible until you scroll;
/// the reconnect fast path (frame_ws_poll.rs) compares the server dialled with it.
///
/// THE OWNER'S DOOR (M2 of the review of the signed requests' round 5, 2026-11-21): every node this
/// page starts also listens on 127.0.0.1 at [`owner_port_for`] its port (the relay's OWNER_PORT),
/// taking its own names there for this app's key alone (relay/audience.rs). The owner's app comes
/// in by it, whatever the node answers to on its main port: round 5 had the app connect by the
/// node's LAN address, but "Add to my servers" saved the loopback address a node with a public
/// address no longer answers to, and with M1 such a node lists no LAN name either. Why a door and
/// not the public name: an onion address or a port forward is often not reachable from the
/// computer itself (no Tor in the app, a router without hairpin), and the app's dozens of ways of
/// talking to a server would each need telling to dial loopback for it; a second loopback port
/// works with every one of them unchanged, and its names mean nothing anywhere else.
pub fn running_local_url() -> Option<String> {
    let n = node().lock().ok()?;
    (n.status == NodeStatus::Running).then(|| owner_url(n.running_port))
}

/// The address "Copy address to share" offers for the running node: one it answers to that someone
/// else may use: the public one, else the network one, else for a node for this computer only its
/// loopback port. Never the owner's door (this app's key alone), and never a network node's
/// loopback port (round 7, 2026-11-21: with no LAN address found it offered http://127.0.0.1:{port},
/// which such a node does not answer to): None then, and the page says the local address is not
/// known yet.
fn share_url_for(n: &LocalNode) -> Option<String> {
    if let Some(p) = public_url(&n.running_public) {
        return Some(p);
    }
    if n.running_local_only {
        return Some(format!("http://127.0.0.1:{}", n.running_port));
    }
    n.lan_ip.as_ref().map(|ip| format!("http://{ip}:{}", n.running_port))
}

/// The first address in a Public address field, as a URL to hand out (`http://` unless the entry
/// names its own scheme); None for an empty field.
fn public_url(field: &str) -> Option<String> {
    let first = field.split(',').map(str::trim).find(|e| !e.is_empty())?;
    Some(if first.contains("://") { first.trim_end_matches('/').to_string() } else { format!("http://{first}") })
}

/// The owner's door's address for a node on `port`: what "Add to my servers" saves and the "On
/// this computer" row shows.
fn owner_url(port: u16) -> String {
    format!("http://127.0.0.1:{}", owner_port_for(port))
}

/// Whether this app's own node is serving now (the Relays page's THIS PC dot).
pub fn node_running() -> bool {
    node().lock().is_ok_and(|n| n.status == NodeStatus::Running)
}

/// What a pass minted by this app names when the server it opened is this app's own running node
/// (C8 and L3, 2026-11-21; engine/dm.rs `pass_audience`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnNodePass {
    /// `opened` is not one of this app's own node's local names: the pass names `opened`.
    NotOwnNode,
    /// The node's first public address: the pass names it.
    Name(String),
    /// The node has no public address: no pass is made there (the words on the Host a node page,
    /// `host_node.public_address_note`, say why).
    NoPass,
}

/// [`OwnNodePass`] for `opened`. A pass names the address its issuer's app opened, but on its own
/// node the owner's app opened a name that is the node's alone to use or that tells where the node
/// is: a loopback name (taken only from this computer, so no friend could present it) or the
/// node's LAN address (handed to every friend, it shows where the household's network is: L3 of the
/// review of rounds 3 and 4). So a pass there names only the node's first Public address, a name
/// the node answers to that a friend elsewhere may use ([`node_public_hosts`]); with none, no pass.
pub fn own_node_pass_name(opened: &str, public_field: &str) -> OwnNodePass {
    let (port, local_only, lan) = {
        let Ok(n) = node().lock() else { return OwnNodePass::NotOwnNode };
        if n.status != NodeStatus::Running {
            return OwnNodePass::NotOwnNode;
        }
        (n.running_port, n.running_local_only, n.lan_ip.clone())
    };
    // Its loopback names on its port and on its owner's door (the address this app opens it by,
    // M2), and its LAN address.
    let door = owner_port_for(port);
    let mut local: Vec<String> = [port, door].iter().flat_map(|p| [format!("127.0.0.1:{p}"), format!("localhost:{p}"), format!("[::1]:{p}")]).collect();
    local.extend(lan.as_deref().and_then(|ip| humanity_protocol::signed_request::canonical_audience(&format!("{ip}:{port}"))));
    if !local.iter().any(|l| l == opened) {
        return OwnNodePass::NotOwnNode;
    }
    pass_name_for(port, local_only, public_field, lan.as_deref()).map_or(OwnNodePass::NoPass, OwnNodePass::Name)
}

/// [`own_node_pass_name`]'s choice for a node on `port`: its first public name, else None (no
/// pass). The port, the reach and the LAN address are no longer part of it (L3: round 4 fell back
/// to the LAN address) and are kept so the test reads the call the page makes.
fn pass_name_for(_port: u16, _local_only: bool, public_field: &str, _lan: Option<&str>) -> Option<String> {
    public_names(public_field).ok().and_then(|p| p.into_iter().next())
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
///
/// Loopback comes FIRST. first_busy stops at the first address it cannot
/// bind, so a port a squatter holds on loopback is reported before the
/// wildcard is ever test-bound: the common failure costs no firewall prompt
/// even for a network node.
fn preflight_addrs(bind_ip: std::net::IpAddr, port: u16) -> Vec<std::net::SocketAddr> {
    let loopback = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let own = std::net::SocketAddr::new(bind_ip, port);
    if own == loopback {
        vec![loopback]
    } else {
        vec![loopback, own]
    }
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

/// Everything a start needs, worked out from the form before anything is
/// touched. Pure (it binds nothing and sets nothing), so the tests can check
/// exactly what a node for each "Who can connect" choice would be told and
/// would test-bind, without starting one.
struct NodePlan {
    port: u16,
    db: String,
    name: String,
    /// The address the relay listens on (its BIND_ADDRESS).
    bind_ip: std::net::IpAddr,
    /// The addresses test-bound before starting (preflight_addrs).
    preflight: Vec<std::net::SocketAddr>,
    /// The relay settings to set (Some) or clear (None), in order. The relay
    /// reads its configuration from the environment, the same knobs the VPS
    /// systemd unit sets, which is what makes the in-app node and the
    /// headless one the same server with the same behaviour.
    env: Vec<(&'static str, Option<String>)>,
}

/// The names a node on `port` answers to on its main port: the Public address field's `public`
/// names (a port forward, a domain, an onion address: C10, 2026-11-21) when it has any, else the
/// names of where it listens (this computer's loopback names for a node for this computer only, its
/// local network address `lan` for a network node). Its owner's app comes in by its own door, never
/// by these ([`running_local_url`]). The relay takes a loopback name only from this computer and a
/// LAN name only from that network, by the connection's own address (relay/audience.rs
/// `peer_may_use`), so the names every home node shares say nothing to a visitor from elsewhere.
///
/// A FORWARDER HANDS VISITORS OVER FROM ITS OWN ADDRESS. Whatever forwards a public name's visitors
/// and adds no header (Tor, a tunnel's agent) gives each the forwarder's address: on this computer
/// that is 127.0.0.1, on the network a local address. So:
/// - no loopback name on a node with a public name, nor on a network node (H1 of the review of
///   rounds 3 and 4, 2026-11-21; the owner's door made them unneeded): a forwarder on this
///   computer hands over visitors who can use only the public names;
/// - no LAN name beside a public one unless the owner asks, `lan_too` (M1 of the review of round
///   5, 2026-11-21: Tor on a router, a Pi or a NAS, Tor pointed at this computer's own LAN address,
///   a tunnel agent on another device, made every visitor local and round 2's pass-through worked
///   again). The page says what that risks where the box is ticked (`host_node.lan_too_risk`).
fn node_public_hosts(port: u16, local_only: bool, lan: Option<&str>, public: &[String], lan_too: bool) -> String {
    let mut names = public.to_vec();
    if local_only && public.is_empty() {
        names.extend([format!("127.0.0.1:{port}"), format!("localhost:{port}")]);
    }
    if let (false, Some(ip)) = (local_only, lan) {
        if public.is_empty() || lan_too {
            names.push(format!("{ip}:{port}"));
        }
    }
    names.join(",")
}

/// The Public address field read as names: each comma-separated entry a `host[:port]` or a URL
/// (its address taken as the app would sign for it), lowercase. The words for the first entry that
/// is not an address otherwise.
fn public_names(field: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for entry in field.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        let name = if entry.contains("://") {
            humanity_protocol::signed_request::audience_of(entry)
        } else {
            humanity_protocol::signed_request::canonical_audience(entry)
        };
        match name {
            Some(n) if !out.contains(&n) => out.push(n),
            Some(_) => {}
            None => return Err(humanity_core::i18n::tf("host_node.public_address_bad", &[("value", &entry)])),
        }
    }
    Ok(out)
}

/// Check the form and work out the plan, or say what is wrong with it.
fn plan_start(n: &LocalNode) -> Result<NodePlan, String> {
    plan_start_for(n, owner_key_for_admin())
}

/// [`plan_start`] for the owner key `owner` (the app's identity, which the page notes each frame):
/// the key the node grants the admin role and the only one its owner's door takes.
fn plan_start_for(n: &LocalNode, owner: Option<String>) -> Result<NodePlan, String> {
    let port: u16 = match n.port_input.trim().parse::<u32>() {
        Ok(p) if (1..=65535).contains(&p) => p as u16,
        _ => {
            return Err(
                "Port must be a whole number between 1 and 65535. 3210 is the usual one."
                    .to_string(),
            )
        }
    };

    let db = n.db_input.trim().to_string();
    if db.is_empty() {
        return Err(
            "Pick a file for the database, for example a relay.db inside a folder you own."
                .to_string(),
        );
    }

    let name = n.name_input.trim().to_string();
    // A node for this computer only may have a public name since M2: Tor or a tunnel on this
    // computer forwarding to it is the setup the docs advise, and its owner's app comes in by the
    // door (round 5 refused it, the owner's app having no name left to use).
    let public = public_names(&n.public_input)?;
    let bind_ip = node_bind_ip(n.local_only);
    let mut env = vec![
        ("PORT", Some(port.to_string())),
        ("DATABASE_PATH", Some(db.clone())),
        // Always set, never inherited: the choice on this page decides who can
        // connect, not whatever BIND_ADDRESS the app happened to be launched
        // with. Without this line a node set to "Only this computer" would
        // listen on every interface (the relay's default); the tests pin it.
        (crate::relay::BIND_ADDRESS_ENV, Some(bind_ip.to_string())),
        // The person hosting the node owns it: their identity gets the admin
        // role in the node's database at startup (operator field test 3: the
        // self-relay's Server Settings showed only the USER section, because
        // nothing had ever granted the owner admin). None clears it.
        (crate::relay::audience::OWNER_KEY_ENV, owner),
        // The addresses this node answers to, which every request and sign-in made for it must name
        // (relay/audience.rs, 2026-11-21): [`node_public_hosts`]. Always set, never inherited, like
        // BIND_ADDRESS. The dev switch for loopback names (HUMANITY_DEV_LOOPBACK_NAMES) is cleared:
        // the names here are the list.
        (crate::relay::audience::PUBLIC_HOSTS_ENV, Some(node_public_hosts(port, n.local_only, lan_address().as_deref(), &public, n.lan_too))),
        (crate::relay::audience::DEV_LOOPBACK_NAMES_ENV, None),
        // The owner's door (M2): the way this app comes in, on 127.0.0.1 at the next port, for the
        // owner's key alone (the key above, HUMANITY_OWNER_ADMIN_KEY).
        (crate::relay::audience::OWNER_PORT_ENV, Some(owner_port_for(port).to_string())),
        // Tailscale reach (L3): only when the owner says this computer is on a Tailscale network.
        (crate::relay::audience::TAILNET_ENV, n.tailnet.then(|| "1".to_string())),
    ];
    if !name.is_empty() {
        env.push(("SERVER_NAME", Some(name.clone())));
    }
    // The door's port is test-bound too, after the main port's addresses (on loopback only, so it
    // raises no firewall prompt): a squatter there is told in words before anything starts.
    let mut preflight = preflight_addrs(bind_ip, port);
    preflight.push(std::net::SocketAddr::from(([127, 0, 0, 1], owner_port_for(port))));
    Ok(NodePlan {
        port,
        db,
        name,
        bind_ip,
        preflight,
        env,
    })
}

/// Validate the form, then bring the node up on its own thread.
///
/// Everything that can be checked cheaply is checked BEFORE the thread exists,
/// because a failure inside `run_relay()` is a panic on a detached thread, and
/// "nothing happened" is the worst possible feedback.
fn start(n: &mut LocalNode) {
    start_with(n, first_busy)
}

/// `start`, with the port check passed in. The app always uses first_busy;
/// a test passes a check that records which addresses it was asked about and
/// answers "busy", so it can prove which addresses start() tests without
/// binding any of them (some are the wildcard, see the note in the tests).
fn start_with(
    n: &mut LocalNode,
    port_check: impl Fn(&[std::net::SocketAddr]) -> Option<(std::net::SocketAddr, std::io::Error)>,
) {
    // Any start, by hand or by autostart, settles a held-back autostart.
    n.autostart_waiting = false;
    let plan = match plan_start(n) {
        Ok(p) => p,
        Err(why) => {
            n.status = NodeStatus::Failed;
            n.message = why;
            return;
        }
    };
    let NodePlan { port, db, name, bind_ip, preflight, env } = plan;

    // The common failure by far: something already holds the port. Test-bind and
    // release it here so the message names the real problem. Which addresses,
    // and why loopback is among them even for a network node: preflight_addrs.
    // Unit tests hold a loopback port and assert it is reported.
    //
    // There is a small window between this check and the relay's own bind in
    // which another program could take the port; catch_unwind below is the net
    // for that rarer case.
    if let Some((busy, e)) = port_check(&preflight) {
        n.status = NodeStatus::Failed;
        n.message = if busy.port() == port {
            format!(
                "Port {port} is not available ({e}). Another program is already using it, \
                 possibly a node you started earlier. Try a different port."
            )
        } else {
            humanity_core::i18n::tf("host_node.door_port_busy", &[("port", &busy.port().to_string()), ("error", &e.to_string())])
        };
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

    // Hand the relay its settings (what each one is for: plan_start).
    apply_env(&env);

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
    n.running_local_only = bind_ip.is_loopback();
    n.running_public = n.public_input.trim().to_string();
    n.running_lan_too = n.lan_too;
    n.started_at = None;
    // A node for this computer only has no network address to hand out (and
    // the LAN probe's UDP socket is not worth opening for it).
    n.lan_ip = if bind_ip.is_loopback() { None } else { lan_address() };
    n.stop_tx = Some(stop_tx);
    n.events = Some(ev_rx);
}

/// Hand the relay the plan's settings: set each one (Some) or clear it (None) in this process's
/// environment, which the relay reads at start (relay/mod.rs `run_relay`, audience.rs `from_env`).
fn apply_env(env: &[(&'static str, Option<String>)]) {
    for (key, value) in env {
        match value {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
    }
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

    Frame::NONE
        .fill(theme.bg_card())
        .stroke(Stroke::new(1.0_f32, theme.border()))
        .corner_radius(CornerRadius::same(theme.border_radius as u8))
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
                &humanity_core::i18n::t("host_node.public_address"),
                &humanity_core::i18n::t("host_node.public_address_hint"),
                &mut n.public_input,
                &humanity_core::i18n::t("host_node.public_address_placeholder"),
            );
            // What a public address changes, said where it is set (H1 and L3, 2026-11-21).
            ui.label(
                RichText::new(humanity_core::i18n::t("host_node.public_address_note"))
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
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
            draw_reach_choices(ui, theme, n, field_w, starting);
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
            let mut auto = state.host_node.autostart;
            if ui
                .checkbox(&mut auto, "Start this node automatically when the app opens")
                .on_hover_text(
                    "Reproduces this exact node (port, database, name, who can connect) at every launch, \
                     so your self-hosted server is reachable again without visiting this page.",
                )
                .changed()
            {
                state.host_node.autostart = auto;
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
                        state.host_node.autostart = true;
                        state.host_node.port = n.port_input.clone();
                        state.host_node.db = n.db_input.clone();
                        state.host_node.name = n.name_input.clone();
                        state.host_node.public = n.public_input.clone();
                        state.host_node.local_only = n.local_only;
                        state.host_node.lan_too = n.lan_too;
                        state.host_node.tailnet = n.tailnet;
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
                    n.public_input.clear();
                    n.local_only = false;
                    n.lan_too = false;
                    n.tailnet = false;
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
                    FilePickerResult::Picked(dirs) => {
                        let Some(dir) = dirs.into_iter().next() else { return };
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

/// "Add to my servers": the running node in the Chat server list, by its owner's door (M2), the
/// one address it always takes this app's key at. Once.
fn add_to_my_servers(state: &mut GuiState, n: &LocalNode) {
    let url = owner_url(n.running_port);
    // The node's own earlier entry by its main port on loopback (round 5 saved that, and a network
    // node no longer answers to it, round 6): replaced in its place by the door, never left beside
    // it (round 7, 2026-11-21), and the server being used follows.
    let norm = crate::gui::pages::chat::norm_server_url;
    let old: Vec<String> = ["127.0.0.1", "localhost", "[::1]"].iter().map(|h| norm(&format!("http://{h}:{}", n.running_port))).collect();
    let was_old = |u: &str| old.contains(&norm(u));
    if was_old(&state.server_url) {
        state.server_url = url.clone();
        state.settings_dirty = true;
    }
    if let Some(i) = state.chat_servers.iter().position(|s| was_old(&s.url)) {
        if state.chat_servers.iter().any(|s| s.url == url) {
            state.chat_servers.remove(i);
        } else {
            state.chat_servers[i].url = url.clone();
            state.chat_servers[i].id = format!("srv_{url}");
        }
        state.settings_dirty = true;
    }
    state.chat_servers.retain(|s| !was_old(&s.url));
    if state.chat_servers.iter().any(|s| s.url == url) {
        return;
    }
    let name = if n.running_name.trim().is_empty() { "My node".to_string() } else { n.running_name.trim().to_string() };
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

/// The two reach choices under "Who can connect", each saying what it changes and what it risks:
/// "Also answer on my local network" beside a public address (M1, 2026-11-21) and "This computer
/// is on a Tailscale network" (L3).
fn draw_reach_choices(ui: &mut egui::Ui, theme: &Theme, n: &mut LocalNode, field_w: f32, starting: bool) {
    use humanity_core::i18n::t;
    let note = |ui: &mut egui::Ui, words: String| {
        ui.label(RichText::new(words).size(theme.font_size_small).color(theme.text_muted()));
    };
    widgets::form_row(ui, theme, "", |ui| {
        ui.vertical(|ui| {
            ui.set_max_width(field_w);
            // Only meaningful with a public address on a network node: without one the LAN address
            // is the node's name anyway, and a node for this computer only has none to give.
            let has_public = !n.public_input.trim().is_empty();
            ui.add_enabled_ui(!starting && has_public && !n.local_only, |ui| {
                ui.checkbox(&mut n.lan_too, t("host_node.lan_too")).hint("host_node.lan_too_hint");
            });
            note(ui, if n.lan_too && has_public { t("host_node.lan_too_risk") } else { t("host_node.lan_too_off") }.to_string());
            ui.add_space(theme.spacing_xs);
            ui.add_enabled_ui(!starting, |ui| {
                ui.checkbox(&mut n.tailnet, t("host_node.tailnet")).hint("host_node.tailnet_hint");
            });
            note(ui, if n.tailnet { t("host_node.tailnet_on") } else { t("host_node.tailnet_off") }.to_string());
        });
    });
}

/// What a serving node looks like: the addresses to hand out, and Stop.
fn draw_running(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, n: &mut LocalNode) {
    // This app's own way in (M2): the owner's door, the one address here the node always answers
    // this app's key at, whatever else it answers to.
    let local_url = owner_url(n.running_port);
    let public_first = public_url(&n.running_public);
    // The LAN address, when the node answers to it (M1: beside a public address only when asked).
    let lan_answered = public_first.is_none() || n.running_lan_too;
    let lan_url = n
        .lan_ip
        .as_ref()
        .map(|ip| format!("http://{ip}:{}", n.running_port));

    Frame::NONE
        .fill(theme.bg_card())
        .stroke(Stroke::new(1.0_f32, theme.border()))
        .corner_radius(CornerRadius::same(theme.border_radius as u8))
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
            widgets::form_row(ui, theme, "", |ui| {
                ui.label(
                    RichText::new(humanity_core::i18n::t("host_node.owner_door_note"))
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            });
            if let Some(p) = &public_first {
                row(ui, &humanity_core::i18n::t("host_node.public_address"), p, true);
            }
            match &lan_url {
                _ if n.running_local_only => row(
                    ui,
                    "On your network",
                    "not shared: this node accepts this computer only",
                    false,
                ),
                _ if !lan_answered => row(ui, "On your network", &humanity_core::i18n::t("host_node.lan_not_answered"), false),
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
                    humanity_core::text::prefix(&n.server_pubkey, 12),
                    humanity_core::text::suffix(&n.server_pubkey, 6)
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
    let public_reach;
    let reach = if public_first.is_some() {
        // Where its visitors must come from (M1): a forwarder pointed at this computer.
        public_reach = humanity_core::i18n::tf("host_node.reach_public", &[("port", &n.running_port.to_string())]);
        public_reach.as_str()
    } else if n.running_local_only {
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
    // An address the node answers to that someone else may use: the public one, else the network
    // one, else (this computer only) its loopback port. Never the owner's door, which takes this
    // app's key alone.
    let share_url = share_url_for(n);
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
            state.host_node.autostart = false;
            crate::config::AppConfig::from_gui_state(state).save();
        }
        if widgets::Button::secondary("Add to my servers")
            .disabled(stopping)
            .tooltip("Puts this node in your Chat server list so you can connect to it like any other.")
            .show(ui, theme)
        {
            add_to_my_servers(state, n);
            state.go(GuiPage::Chat, crate::gui::How::Beside);
        }
        if widgets::Button::ghost("Copy address to share").disabled(share_url.is_none()).show(ui, theme) {
            if let Some(u) = &share_url {
                ui.ctx().copy_text(u.clone());
            }
        }
    });
    // A network node whose local address is not known yet has none to share (round 7).
    if share_url.is_none() {
        ui.label(RichText::new(humanity_core::i18n::t("host_node.share_unknown")).size(theme.font_size_small).color(theme.text_muted()));
    }

    ui.add_space(theme.spacing_sm);
    // The owner review's finding 8: the first-admin claim is gone; whoever holds the claim code
    // from this node's own log becomes its owner.
    ui.label(RichText::new(humanity_core::i18n::t("owner.node_claim_hint")).size(theme.font_size_small).color(theme.text_muted()));
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
    // checking WHICH addresses are tested without binding them: preflight_addrs
    // directly, and start() through a port check that records its question
    // and answers "busy" (start_with). The autostart tests use a saved node
    // for this computer only on a held loopback port, so even a regression
    // that starts it never listens.

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
    // [0.0.0.0:3210]". And again after the critic review, against the order
    // the first version used ([0.0.0.0:PORT, 127.0.0.1:PORT]): "loopback is
    // tested FIRST, so a loopback squatter is found before the wildcard is
    // test-bound / left: 0.0.0.0:3210 / right: 127.0.0.1:3210". Pure: nothing
    // is bound.
    #[test]
    fn preflight_for_the_network_tests_loopback_first_then_its_own_address() {
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
        assert_eq!(
            addrs[0],
            std::net::SocketAddr::from(([127, 0, 0, 1], 3210)),
            "loopback is tested FIRST, so a loopback squatter is found before the wildcard is \
             test-bound"
        );
    }

    /// A database path under the temp folder, so a regression that gets past
    /// the port check cannot write into the real profile directory.
    fn scratch_db() -> String {
        std::env::temp_dir()
            .join("humanity-host-node-test")
            .join("relay.db")
            .display()
            .to_string()
    }

    // What start() hands the relay. Without the BIND_ADDRESS pair a node set
    // to "Only this computer" would listen on every interface (the relay's
    // default), and every other test here would still pass: the end-to-end
    // one stops at the port check, before any setting is handed over.
    //
    // Red first, 2026-10-03 (critic review), with the BIND_ADDRESS pair
    // deleted from plan_start: "assertion `left == right` failed: \"Only this
    // computer\" must tell the relay to listen on loopback / left: None /
    // right: Some(\"127.0.0.1\")". Pure: nothing is bound or set.
    #[test]
    fn the_plan_tells_the_relay_who_can_connect() {
        let plan_for = |local_only: bool| {
            let mut n = LocalNode::new();
            n.local_only = local_only;
            n.port_input = "3210".to_string();
            n.db_input = scratch_db();
            plan_start(&n).expect("a valid form")
        };
        let setting = |plan: &NodePlan, key: &str| {
            plan.env.iter().find(|(k, _)| *k == key).and_then(|(_, v)| v.clone())
        };

        let local = plan_for(true);
        assert_eq!(
            setting(&local, crate::relay::BIND_ADDRESS_ENV).as_deref(),
            Some("127.0.0.1"),
            "\"Only this computer\" must tell the relay to listen on loopback"
        );
        assert!(local.bind_ip.is_loopback());

        let network = plan_for(false);
        assert_eq!(
            setting(&network, crate::relay::BIND_ADDRESS_ENV).as_deref(),
            Some("0.0.0.0"),
            "\"Devices on my network\" must tell the relay to listen on every interface"
        );
        for plan in [&local, &network] {
            assert_eq!(setting(plan, "PORT").as_deref(), Some("3210"));
            assert_eq!(setting(plan, "DATABASE_PATH"), Some(scratch_db()));
            // The names the node answers to (2026-11-21). Red evidence: without the PUBLIC_HOSTS pair,
            // None here, and a node on every interface refused its own owner's sign-in.
            let names = setting(plan, crate::relay::audience::PUBLIC_HOSTS_ENV).expect("the node is told its names");
            // The owner's door, the way this app comes in (M2, 2026-11-21). Red evidence: without the
            // OWNER_PORT pair, "every node opens its owner's door" failed, left None.
            assert_eq!(setting(plan, crate::relay::audience::OWNER_PORT_ENV).as_deref(), Some("3211"), "every node opens its owner's door");
            // Red evidence (2026-11-21, round 5's main-port names put back, loopback names on a
            // network node): failed for the network plan, left true, right false.
            let loopback = names.split(',').any(|n| n == "127.0.0.1:3210");
            assert_eq!(loopback, plan.bind_ip.is_loopback(), "a loopback name on the main port for this computer only, never on a network node: {names}");
        }
    }

    /// A node with no public address answers to where it listens: a network node to its LAN address
    /// alone (its owner's app comes in by the door, M2, so no loopback name on the main port a
    /// forwarder on this computer could hand visitors to), a node for this computer only to its
    /// loopback names. Red evidence (2026-11-21, round 5's list put back): "a network node lists no
    /// loopback name" failed, left "127.0.0.1:3210,localhost:3210,192.168.1.5:3210".
    #[test]
    fn a_network_node_answers_to_its_local_address() {
        assert_eq!(node_public_hosts(3210, false, Some("192.168.1.5"), &[], false), "192.168.1.5:3210", "a network node lists no loopback name");
        assert_eq!(node_public_hosts(3210, true, Some("192.168.1.5"), &[], false), "127.0.0.1:3210,localhost:3210", "a node for this computer only names no other");
        assert_eq!(node_public_hosts(4000, false, None, &[], false), "", "no LAN address found: no name but the door");
    }

    /// H1 (the review of rounds 3 and 4, 2026-11-21) and M1 (the review of round 5, the same day): a
    /// node with a public address (a port forward, a domain, an onion address) lists no loopback name,
    /// because whatever forwards its visitors on this computer hands them over from 127.0.0.1, and no
    /// LAN name unless its owner ticks "Also answer on my local network", because a forwarder on the
    /// network that adds no header hands them over from a local address. A node for this computer
    /// only may have one now (Tor on this computer pointed at it, the setup the docs advise): its
    /// owner's app comes in by the door (M2).
    /// Red evidence: (2026-11-21, before H1) "a node with a public address lists no loopback name"
    /// failed, left "abcdefghijklmnop.onion,127.0.0.1:3210,localhost:3210,192.168.1.5:3210"; (M1,
    /// round 5's rule put back, the LAN address listed whenever the node is on the network) "no LAN
    /// name beside a public one unless asked" failed, left "abcdefghijklmnop.onion,192.168.1.5:3210".
    #[test]
    fn a_node_with_a_public_address_lists_no_loopback_name() {
        let onion = ["abcdefghijklmnop.onion".to_string()];
        assert_eq!(node_public_hosts(3210, false, Some("192.168.1.5"), &onion, false), "abcdefghijklmnop.onion", "no LAN name beside a public one unless asked");
        assert_eq!(node_public_hosts(3210, false, Some("192.168.1.5"), &onion, true), "abcdefghijklmnop.onion,192.168.1.5:3210", "the owner asked");
        assert_eq!(node_public_hosts(3210, true, Some("192.168.1.5"), &onion, true), "abcdefghijklmnop.onion", "for this computer only: the public name alone");
        let mut n = LocalNode::new();
        n.port_input = "3210".to_string();
        n.db_input = scratch_db();
        n.public_input = "abcdefghijklmnop.onion".to_string();
        n.local_only = true;
        let plan = plan_start(&n).expect("a node for this computer only may have a public address");
        assert!(plan.bind_ip.is_loopback());
    }

    /// C8 (2026-11-21), L3 (the review of rounds 3 and 4, the same day): a pass the owner's app mints
    /// on its own node names the node's first Public address, a name the node answers to that a
    /// friend elsewhere may use, and nothing else: with no public address no pass is made there, so
    /// the node's LAN address (or another of its names) is never handed to every friend. The choice
    /// is one of the names the node is told ([`node_public_hosts`]).
    /// Red evidence: with `pass_name_for` returning None always, "the first public address" failed,
    /// left None; with round 4's LAN fallback (L3), "no public address, no pass" failed, left
    /// Some("192.168.1.5:3210").
    #[test]
    fn a_pass_minted_on_the_own_node_names_only_its_public_address() {
        let public = "Home.Example:8443, http://other.example";
        assert_eq!(pass_name_for(3210, false, public, Some("192.168.1.5")).as_deref(), Some("home.example:8443"), "the first public address");
        assert_eq!(pass_name_for(3210, false, "", Some("192.168.1.5")), None, "no public address, no pass");
        assert_eq!(pass_name_for(3210, true, "", Some("192.168.1.5")), None, "a node for this computer only makes none");
        assert_eq!(pass_name_for(3210, false, "", None), None);
        let told = node_public_hosts(3210, false, Some("192.168.1.5"), &public_names(public).unwrap(), false);
        let chosen = pass_name_for(3210, false, public, Some("192.168.1.5")).unwrap();
        assert!(told.split(',').any(|n| n == chosen), "{chosen} is one of the node's names: {told}");
    }

    /// C10 (2026-11-21, signed requests round 3): the Public address field sets the names the
    /// relay answers to, read the way the relay reads them; junk is refused in words; the dev
    /// switch for loopback names is cleared. Red evidence (2026-11-21, before the field existed:
    /// `public_names` and its plan line absent): "the field's names reach the relay" failed, the
    /// relay's names held only the loopback pair.
    #[test]
    fn the_public_address_field_sets_the_relays_names() {
        let mut n = LocalNode::new();
        n.port_input = "3210".to_string();
        n.db_input = scratch_db();
        n.public_input = " Node.Example.org:3210 , https://abcdefghijklmnop.onion/ ,node.example.org:3210".to_string();
        let plan = plan_start(&n).expect("a valid form");
        let names = plan.env.iter().find(|(k, _)| *k == crate::relay::audience::PUBLIC_HOSTS_ENV).and_then(|(_, v)| v.clone()).unwrap();
        let own = crate::relay::audience::OwnNames::from_settings(
            |k| (k == crate::relay::audience::PUBLIC_HOSTS_ENV).then(|| names.clone()),
            "0.0.0.0:3210".parse().unwrap(),
        );
        assert!(own.accepts("node.example.org:3210") && own.accepts("abcdefghijklmnop.onion"), "the field's names reach the relay: {names}");
        assert_eq!(names.matches("node.example.org").count(), 1, "once each: {names}");
        assert!(!names.contains("127.0.0.1") && !names.contains("localhost"), "no loopback name beside a public one (H1): {names}");
        // Red evidence (2026-11-21, round 5's rule put back, M1): failed, left 3 (the LAN address
        // listed), right 2.
        assert_eq!(names.split(',').count(), 2, "and no LAN name unless asked (M1): {names}");
        assert!(plan.env.iter().any(|(k, v)| *k == crate::relay::audience::DEV_LOOPBACK_NAMES_ENV && v.is_none()), "the dev switch is cleared");
        n.public_input = "node.example.org, not an address".to_string();
        let words = plan_start(&n).err().expect("junk is refused");
        assert!(words.contains("not an address"), "{words}");
    }

    // start() must test the addresses the plan lists, loopback first, and not
    // some shorter list of its own. Dropping loopback for a network node is
    // the Windows squatter regression, and no other test sees it: the
    // end-to-end test runs "Only this computer", whose list is loopback alone.
    // The check passed in records what it is asked and answers "busy", so this
    // binds NOTHING, in the green run and in the red one. (An end-to-end
    // version with a real squatter cannot be seen red without listening on
    // 0.0.0.0: every way to make it fail test-binds the wildcard and then
    // starts a relay there, which is the firewall prompt this file avoids.)
    //
    // Red first, 2026-10-03 (critic review), with start_with checking only
    // [SocketAddr::new(bind_ip, port)]: "assertion `left == right` failed:
    // start() must test loopback first (local_only = false) / left:
    // Some(0.0.0.0:3210) / right: Some(127.0.0.1:3210)".
    #[test]
    fn start_tests_the_planned_addresses_loopback_first() {
        let loopback = std::net::SocketAddr::from(([127, 0, 0, 1], 3210));
        let wildcard = std::net::SocketAddr::from(([0, 0, 0, 0], 3210));
        for local_only in [true, false] {
            let mut n = LocalNode::new();
            n.port_input = "3210".to_string();
            n.local_only = local_only;
            n.db_input = scratch_db();
            let asked = std::cell::RefCell::new(Vec::new());
            start_with(&mut n, |addrs| {
                asked.borrow_mut().extend_from_slice(addrs);
                Some((
                    addrs[0],
                    std::io::Error::new(std::io::ErrorKind::AddrInUse, "held by this test"),
                ))
            });
            let asked = asked.into_inner();
            assert_eq!(
                asked.first(),
                Some(&loopback),
                "start() must test loopback first (local_only = {local_only})"
            );
            // The owner's door's port is tested last, on loopback too (M2).
            let door = std::net::SocketAddr::from(([127, 0, 0, 1], 3211));
            assert_eq!(asked.last(), Some(&door), "the owner's door is tested too: {asked:?}");
            if local_only {
                assert_eq!(asked, vec![loopback, door], "a node for this computer only tests loopback alone");
            } else {
                assert!(asked.contains(&wildcard), "a network node tests its own address too: {asked:?}");
            }
            assert_eq!(n.status, NodeStatus::Failed, "a busy answer stops the start");
            assert!(n.message.contains("3210"), "the message names the port: {}", n.message);
            assert!(n.events.is_none(), "nothing should have been spawned");
        }
    }

    /// The saved node of a person who hosts for this computer only, on a port
    /// this test holds on loopback: if autostart starts it at all, the port
    /// check reports the held port (status Failed), so "did it try?" is
    /// visible, and nothing ever listens, let alone on the wildcard.
    fn saved_node_on(port: u16) -> GuiState {
        let mut s = GuiState::default();
        s.host_node.autostart = true;
        s.host_node.port = port.to_string();
        s.host_node.db = scratch_db();
        s.host_node.name = "Test node".to_string();
        s.host_node.local_only = true;
        s
    }

    fn held_loopback_port() -> (std::net::TcpListener, u16) {
        let held = std::net::TcpListener::bind(std::net::SocketAddr::from(([127, 0, 0, 1], 0)))
            .expect("bind a scratch loopback port");
        let port = held.local_addr().unwrap().port();
        (held, port)
    }

    // A copy of the app a SCRIPT launched (an agent's rig, a boot check, a
    // worktree build) must not start the operator's saved node: his is a
    // network node, and starting it from a new exe path is a Windows Firewall
    // prompt in front of him (critic review, 2026-10-03: his config has
    // host_node_autostart on, port 3210, network mode).
    //
    // Red first, 2026-10-03, with autostart ignoring `background`:
    // "assertion `left == right` failed: a script-launched copy started the
    // saved node / left: Failed / right: Stopped" (it tried, and the held
    // port stopped it).
    #[test]
    fn a_script_launch_holds_the_saved_node_back() {
        let (held, port) = held_loopback_port();
        let state = saved_node_on(port);
        let mut n = LocalNode::new();
        autostart(&mut n, &state, true);
        assert_eq!(n.status, NodeStatus::Stopped, "a script-launched copy started the saved node");
        assert!(n.events.is_none());
        assert!(n.autostart_waiting, "it waits for a person instead");
        assert!(n.message.contains("click into the window"), "and says so: {}", n.message);
        assert_eq!(n.port_input, port.to_string(), "the saved node is loaded into the form");
        drop(held);
    }

    // The person's half of the bargain: their first click into the window
    // starts the node that was held back, once.
    //
    // Red first, 2026-10-03, with resume_held_autostart doing nothing:
    // "assertion `left == right` failed: the first click must start the
    // held-back node / left: Stopped / right: Failed".
    #[test]
    fn the_first_click_starts_a_held_back_node_once() {
        let (held, port) = held_loopback_port();
        let state = saved_node_on(port);
        let mut n = LocalNode::new();
        autostart(&mut n, &state, true);
        resume_held_autostart(&mut n, &state);
        assert_eq!(n.status, NodeStatus::Failed, "the first click must start the held-back node");
        assert!(n.message.contains(&port.to_string()), "it tried the saved port: {}", n.message);
        assert!(!n.autostart_waiting, "and is no longer waiting");

        // A later click does nothing: the status and message stay as they are.
        n.message = "untouched".to_string();
        resume_held_autostart(&mut n, &state);
        assert_eq!(n.message, "untouched");
        drop(held);
    }

    // A launch by a person (a double-click, `just launch`) still brings the
    // saved node back at once, the field-test fix autostart exists for.
    //
    // Red first, 2026-10-03, with autostart holding back every launch:
    // "assertion `left == right` failed: a person's launch must start the
    // saved node / left: Stopped / right: Failed".
    #[test]
    fn a_person_launch_still_starts_the_saved_node() {
        let (held, port) = held_loopback_port();
        let state = saved_node_on(port);
        let mut n = LocalNode::new();
        autostart(&mut n, &state, false);
        assert_eq!(n.status, NodeStatus::Failed, "a person's launch must start the saved node");
        assert!(n.message.contains(&port.to_string()), "it tried the saved port: {}", n.message);
        assert!(!n.autostart_waiting);
        drop(held);
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

#[cfg(test)]
#[path = "host_node_owner_tests.rs"]
mod owner_tests;
