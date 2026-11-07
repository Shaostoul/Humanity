// The desktop app's WebRTC against a REAL browser. Run it with `just verify-browser-webrtc` (it is
// #[ignore]d: it needs node and an installed Chromium, Chrome or Edge, which CI does not have).
// It lives with the GUI's crossing tests (mod.rs there says why) because it reaches three crates
// that may not reach each other: the network side (the app's WebRTC and file link), the server
// (its forwarder and credentials) and the engine's reader of a file's credentials.
//
// WHY (2026-11-07). Commit 78072c92c (the security cadence, 2026-11-06) moved the calls' and the
// device-to-device files' WebRTC from str0m 0.20 to 0.25, whose `rust-crypto` backend does DTLS
// through dimpl 0.9 on RustCrypto p256/p384 (its certificate made by rcgen on AWS-LC:
// docs/reference/cryptography.md): a different DTLS stack from 0.20's. The call and file code
// compiled unchanged, and every other test is str0m against str0m (webrtc_call_mode_tests.rs,
// file_link_tests.rs), so nothing had checked the DTLS handshake, the SRTP keys or the SCTP data
// channel against a browser's own stack (Chromium's BoringSSL DTLS). A person on the website calling
// a person on the desktop app, or sending them a file, meets exactly that.
//
// WHAT RUNS, AND WHOSE CODE IT IS:
//  - The desktop side is the app's own: `WebrtcManager` (the call path, net/webrtc.rs) and
//    `FileLink` (the device-to-device files, net/file_link.rs), driven the way the engine drives
//    them (engine/frame_ws_poll.rs for a call's signals, engine/file_transfer.rs for a file's): the
//    call credentials read with `call_relay::parse_reply`, a file's with
//    `engine::file_transfer::read_credentials`, a file offer read with `file_offer::parse_note` and
//    its pass checked with `file_offer::verify_file_pass`, the app's Opus encoder and decoder
//    (net/voice.rs) for the sound.
//  - The browser side is the WEBSITE'S own code, unchanged, in a headless Chromium
//    (scripts/browser-webrtc.js): chat-voice-calls.js, chat-voice-rooms.js and call-connection.js
//    for a call (startCall, acceptIncomingCall, its RTCPeerConnection built from the credentials,
//    relay only), chat-file-transfer.js, file-transfer.js and blake3.js for a file (offerFileDirect,
//    ingestFileNote, Accept, its dial and answer, the Sender and the Receiver, the save), with
//    pq.js signing and checking the file passes for real. Stood in (scripts/browser-webrtc/
//    stand-ins.js): the sign-in, the sealing of DMs, the metadata cleaning, the rows on screen.
//  - Between them, the relay's own forwarder: `call_forwarder::serve` over `Forwarder`, exactly the
//    loop `call_forwarder::start` runs, on 127.0.0.1, with credentials minted by the relay's own
//    `CallKeys` (call and file scopes), every frame to the page in the relay's own wire shape
//    (`RelayMessage`). Relay-only ICE on both sides, as the product runs: each side's ONE candidate
//    is the address the forwarder gave it. What THIS test plays is the relay's chat socket: it
//    routes the frames between the page and the desktop side as the relay would, checks a dial's
//    file pass with the relay's own `pq_crypto::verify_file_pass` and names its transfer as `file`.
//
// THE FOUR SHAPES, each judged on what the far end measured:
//  1. The browser calls the desktop (the browser offers, the desktop answers).
//  2. The desktop calls the browser (the desktop offers, the browser answers). Whichever offers,
//     the browser is the DTLS client and the app the server, as with every browser the product
//     meets: str0m answers an offer `setup:passive`, and Chromium answers one `setup:active`. So
//     the app's DTLS client role (dimpl as client) is met only by another desktop app, never by a
//     browser, and is not exercised here (the call tests cover it, str0m against str0m).
//     For each call: the browser's own getStats says the DTLS transport is `connected`, on a
//     relay-to-relay path, with inbound audio packets arriving and their audio energy rising (the
//     desktop's 440 Hz tone); the desktop decoded audio from the browser that is not silence (the
//     browser's fake microphone beep). SRTP keys come only from the DTLS handshake, so audio decoded
//     at each end is that end's proof the handshake finished.
//  3. A file from the browser to the desktop: the website's sender, the app's receiver; the
//     desktop's file has the offered BLAKE3 and exactly the bytes the page made.
//  4. A file from the desktop to the browser: the app's sender, the website's receiver; the file the
//     browser saved (its download) has exactly the bytes the desktop sent. It runs straight after 3,
//     as two files back to back between the same two people do: the first runs of this check found
//     the website opening the second file's channel on the connection the desktop had just closed
//     (BUG-274, fixed in chat-file-transfer.js).
//
// What it does NOT cover: the real relay process (its chat socket, sign-in, reach rules, and the
// forwarder's settings and socket set-up in `call_forwarder::start`; the forwarder's protocol and
// serving loop are the real ones); the sealed DMs the offers ride in; voice ROOMS (the same audio
// machinery as a call, with the room's own signalling); the Direct call modes; the app's DTLS
// client role against a browser (above); Firefox and Safari; a real network between the two (the
// forwarder, both apps and the browser's relayed addresses are all on 127.0.0.1, which needs the
// browser's --allow-loopback-in-peer-connection, scripts/browser-webrtc.js says why).
//
// RED FIRST: HOS_BROWSER_WEBRTC_BREAK breaks one thing on purpose, so the judge is seen failing:
//   fingerprint  every SDP from the browser reaches the desktop with its DTLS fingerprint changed
//   forwarder    the browser's forwarder credentials are for another call or transfer, so the
//                forwarder passes nothing between the two
//   hash         each offer's whole-file BLAKE3 is another file's
//   silence      the desktop sends silence and the page mutes its microphone (toggleMute)
// The red runs, 2026-11-07, are recorded at the end of this file.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::net::call_relay::{self, CallScope, Reply};
use crate::net::file_link::{signal_text, Cmd, FileLink, LinkEvent, Setup, Side, Transfer};
use crate::net::file_offer::{self, FilePass, Offer, PIECE_SIZE};
use crate::net::file_transfer::{End, Receiving, Sending};
use crate::net::voice;
use crate::net::webrtc::{WebrtcEvent, WebrtcHandle, WebrtcManager, CALL_ROOM_ID};
use crate::relay::call_credentials::{CallCredentials as RelayCredentials, CallService, CREDENTIAL_TTL_SECS};
use crate::relay::call_forwarder::{serve, Forwarder};
use crate::relay::relay::RelayMessage;

/// A line of the run's story, with the seconds since the check began (`START`).
macro_rules! say {
    ($($arg:tt)*) => {
        println!("{:6.2}s {}", START.get_or_init(Instant::now).elapsed().as_secs_f64(), format!($($arg)*))
    };
}

/// When the check began, for `say!`.
static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// The server DID both sides sign and check file passes for (a pass names the server it is for).
const SERVER_DID: &str = "did:hum:browserwebrtccheck";
/// How long a call may take to connect and carry sound both ways.
const CALL_WAIT: Duration = Duration::from_secs(30);
/// How long a file may take, offer to saved.
const FILE_WAIT: Duration = Duration::from_secs(60);
/// How much of the desktop's tone has gone out before the browser's statistics are read.
const TONE_BEFORE_STATS: u64 = 150; // 20 ms frames: 3 s
/// A decoded 20 ms frame louder than this (RMS, of 32767) counts as sound, not silence.
const LOUD_RMS: f64 = 300.0;
/// How many of the browser's frames must be sound: its fake microphone beeps, about 40 loud frames
/// in a call's 4.5 s (2026-11-07); muted after the first moments, 4 got through.
const MIN_LOUD_FRAMES: u64 = 15;
/// How much the browser's inbound audio energy must rise between its two readings, about 1.5 s
/// apart: the desktop's tone raised it about 0.09 each time (2026-11-07); silence, by next to nothing.
const MIN_ENERGY_RISE: f64 = 0.02;

/// What a red run breaks (HOS_BROWSER_WEBRTC_BREAK).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Break {
    Nothing,
    Fingerprint,
    Forwarder,
    Hash,
    Silence,
}

impl Break {
    fn from_env() -> Break {
        match std::env::var("HOS_BROWSER_WEBRTC_BREAK").unwrap_or_default().as_str() {
            "" => Break::Nothing,
            "fingerprint" => Break::Fingerprint,
            "forwarder" => Break::Forwarder,
            "hash" => Break::Hash,
            "silence" => Break::Silence,
            other => panic!("HOS_BROWSER_WEBRTC_BREAK={other:?}: one of fingerprint, forwarder, hash, silence"),
        }
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// The made-up bytes of a test file (harness.js `fileBytes` makes the same).
fn file_bytes(len: usize, seed: u32) -> Vec<u8> {
    (0..len).map(|i| (((i as u32).wrapping_mul(0x9E37_79B1) >> 13) ^ seed) as u8).collect()
}

/// An SDP with its DTLS fingerprint changed (one hex digit), for the red run.
fn tamper_fingerprint(sdp: &str) -> String {
    let Some(at) = sdp.find("a=fingerprint:") else { return sdp.to_string() };
    let Some(space) = sdp[at..].find(' ') else { return sdp.to_string() };
    let digit = at + space + 1;
    let mut out = sdp.to_string();
    let flipped = if &sdp[digit..digit + 1] == "0" { "1" } else { "0" };
    out.replace_range(digit..digit + 1, flipped);
    out
}

/// A signal's data with its SDP's fingerprint changed: an object with `sdp` (a call's), or a JSON
/// string of one (a file link's).
fn tamper_signal(data: &Value) -> Value {
    match data {
        Value::String(text) => match serde_json::from_str::<Value>(text) {
            Ok(inner) if inner.get("sdp").is_some() => Value::String(tamper_signal(&inner).to_string()),
            _ => data.clone(),
        },
        Value::Object(_) => {
            let mut v = data.clone();
            if let Some(sdp) = v.get("sdp").and_then(Value::as_str).map(tamper_fingerprint) {
                v["sdp"] = Value::String(sdp);
            }
            v
        }
        _ => data.clone(),
    }
}

/// A signal's gist for the log: an SDP's DTLS role and fingerprint hash and its candidates' kinds,
/// or a candidate's kind and address.
fn signal_note(data: &Value) -> String {
    let inner = match data {
        Value::String(text) => serde_json::from_str::<Value>(text).unwrap_or(Value::Null),
        other => other.clone(),
    };
    if let Some(sdp) = inner.get("sdp").and_then(Value::as_str) {
        let setup = sdp.lines().find_map(|l| l.strip_prefix("a=setup:")).unwrap_or("?");
        let hash = sdp.lines().find_map(|l| l.strip_prefix("a=fingerprint:")).and_then(|f| f.split(' ').next()).unwrap_or("?");
        let m: Vec<&str> = sdp.lines().filter_map(|l| l.strip_prefix("m=")).map(|l| l.split(' ').next().unwrap_or("?")).collect();
        let cands: Vec<String> = sdp
            .lines()
            .filter(|l| l.starts_with("a=candidate:"))
            .map(|l| l.split(' ').skip(4).take(4).collect::<Vec<_>>().join(" "))
            .collect();
        return format!(": setup {setup}, fingerprint {hash}, m-lines {m:?}, candidates {cands:?}");
    }
    if let Some(c) = inner.get("candidate").and_then(Value::as_str) {
        return format!(": {}", c.split(' ').skip(4).take(4).collect::<Vec<_>>().join(" "));
    }
    String::new()
}

// ── The page's files and its socket, on 127.0.0.1 ─────────────────────────────────────────────

/// The file a URL of the page names: the harness from scripts/browser-webrtc/, the website's own
/// scripts from web/ (as the site serves them), the data files from data/.
fn page_file(url: &str) -> Option<PathBuf> {
    let root = Path::new(env!("HUMANITY_REPO_ROOT"));
    if url.contains("..") || url.contains('\\') || url.contains(':') {
        return None;
    }
    if url == "/" {
        return Some(root.join("scripts/browser-webrtc/harness.html"));
    }
    let (base, rest) = [("/harness/", "scripts/browser-webrtc"), ("/shared/", "web/shared"), ("/chat/", "web/chat"), ("/data/", "data")]
        .into_iter()
        .find_map(|(prefix, dir)| url.strip_prefix(prefix).map(|rest| (dir, rest)))?;
    Some(root.join(base).join(rest))
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("json") => "application/json",
        Some("css") => "text/css",
        _ => "application/octet-stream",
    }
}

/// Answer one HTTP request for a file of the page.
fn serve_file(mut s: TcpStream) {
    let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
    let mut head = Vec::new();
    let mut chunk = [0u8; 4096];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") {
        match s.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => head.extend_from_slice(&chunk[..n]),
        }
        if head.len() > 65_536 {
            return;
        }
    }
    let text = String::from_utf8_lossy(&head);
    let url = text.split_whitespace().nth(1).unwrap_or("/").split('?').next().unwrap_or("/").to_string();
    let found = page_file(&url).and_then(|p| std::fs::read(&p).ok().map(|b| (content_type(&p), b)));
    let (status, ctype, body) = match found {
        Some((ctype, body)) => ("200 OK", ctype, body),
        None => ("404 Not Found", "text/plain", format!("not here: {url}").into_bytes()),
    };
    let header = format!("HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n", body.len());
    let _ = s.write_all(header.as_bytes());
    let _ = s.write_all(&body);
}

/// The page's chat socket: one WebSocket at a time, nonblocking, read and written by one thread.
fn socket_loop(listener: TcpListener, tx: Sender<Value>, rx: Receiver<String>) {
    use tungstenite::{Error, Message};
    let would_block = |e: &std::io::Error| e.kind() == std::io::ErrorKind::WouldBlock;
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let Ok(mut ws) = tungstenite::accept(stream) else { continue };
        if ws.get_ref().set_nonblocking(true).is_err() {
            continue;
        }
        loop {
            let mut moved = false;
            match ws.read() {
                Ok(Message::Text(text)) => {
                    moved = true;
                    if let Ok(v) = serde_json::from_str::<Value>(&text) {
                        if tx.send(v).is_err() {
                            return;
                        }
                    }
                }
                Ok(Message::Close(_)) => break,
                Ok(_) => moved = true,
                Err(Error::Io(e)) if would_block(&e) => {}
                Err(_) => break,
            }
            loop {
                match rx.try_recv() {
                    Ok(text) => {
                        moved = true;
                        let _ = ws.write(Message::Text(text));
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return,
                }
            }
            match ws.flush() {
                Ok(()) => {}
                Err(Error::Io(e)) if would_block(&e) => {}
                Err(_) => break,
            }
            if !moved {
                thread::sleep(Duration::from_millis(2));
            }
        }
    }
}

struct Hub {
    http: SocketAddr,
    ws: SocketAddr,
    from_page: Receiver<Value>,
    to_page: Sender<String>,
}

impl Hub {
    fn start() -> Hub {
        let http = TcpListener::bind("127.0.0.1:0").expect("the page's server on loopback");
        let ws = TcpListener::bind("127.0.0.1:0").expect("the page's socket on loopback");
        let (http_addr, ws_addr) = (http.local_addr().unwrap(), ws.local_addr().unwrap());
        assert!(http_addr.ip().is_loopback() && ws_addr.ip().is_loopback());
        thread::spawn(move || {
            for s in http.incoming().flatten() {
                thread::spawn(move || serve_file(s));
            }
        });
        let (tx_in, from_page) = mpsc::channel();
        let (to_page, rx_out) = mpsc::channel();
        thread::spawn(move || socket_loop(ws, tx_in, rx_out));
        Hub { http: http_addr, ws: ws_addr, from_page, to_page }
    }

    fn page_url(&self) -> String {
        format!("http://{}/?ws=ws://{}/", self.http, self.ws)
    }
}

// ── The browser (scripts/browser-webrtc.js) ─────────────────────────────────────────────────

/// The node script driving the headless browser. Dropping it closes the script's stdin, which
/// closes the browser (and deletes its profile); a script that does not exit in 20 s is killed.
struct Browser {
    child: Child,
}

impl Browser {
    fn start(url: &str, downloads: &Path) -> Browser {
        let script = Path::new(env!("HUMANITY_REPO_ROOT")).join("scripts/browser-webrtc.js");
        let child = Command::new("node")
            .arg(script)
            .arg("--url")
            .arg(url)
            .arg("--downloads")
            .arg(downloads)
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("node runs scripts/browser-webrtc.js");
        Browser { child }
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        drop(self.child.stdin.take());
        let until = Instant::now() + Duration::from_secs(20);
        while Instant::now() < until {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
        let _ = self.child.kill();
    }
}

// ── The desktop's microphone and speaker, stood in ──────────────────────────────────────────

/// The desktop side's sound: a 440 Hz tone out (silence in the red run) through the app's Opus
/// encoder, and what the browser sends, through the app's Opus decoder, measured.
struct Voice {
    encoder: voice::Encoder,
    decoder: voice::Decoder,
    silent: bool,
    connected: bool,
    next_send: Option<Instant>,
    phase: f64,
    sent: u64,
    heard: u64,
    loud: u64,
    loudest: f64,
}

impl Voice {
    fn new(silent: bool) -> Voice {
        Voice {
            encoder: voice::Encoder::new().expect("the app's Opus encoder"),
            decoder: voice::Decoder::new().expect("the app's Opus decoder"),
            silent,
            connected: false,
            next_send: None,
            phase: 0.0,
            sent: 0,
            heard: 0,
            loud: 0,
            loudest: 0.0,
        }
    }

    fn reset(&mut self) {
        *self = Voice::new(self.silent);
    }

    /// One frame from the browser: decoded, and its loudness kept.
    fn hear(&mut self, opus: &[u8]) {
        let mut pcm = vec![0i16; voice::FRAME];
        let Some(n) = self.decoder.decode(opus, &mut pcm) else { return };
        if n == 0 {
            return;
        }
        let rms = (pcm[..n].iter().map(|&s| (s as f64) * (s as f64)).sum::<f64>() / n as f64).sqrt();
        self.heard += 1;
        if rms > LOUD_RMS {
            self.loud += 1;
        }
        self.loudest = self.loudest.max(rms);
    }

    /// While the call is up: one 20 ms frame to `peer` whenever one is due.
    fn tick(&mut self, app: &WebrtcHandle, peer: &str) {
        let Some(due) = self.next_send else { return };
        let now = Instant::now();
        if now < due {
            return;
        }
        let mut pcm = [0i16; voice::FRAME];
        if !self.silent {
            for s in pcm.iter_mut() {
                *s = (self.phase.sin() * 8000.0) as i16;
                self.phase += 2.0 * std::f64::consts::PI * 440.0 / voice::SR as f64;
            }
            self.phase %= 2.0 * std::f64::consts::PI;
        }
        let mut out = [0u8; 1500];
        if let Some(len) = self.encoder.encode(&pcm, &mut out) {
            app.send_voice(peer.to_string(), out[..len].to_vec());
            self.sent += 1;
        }
        // Behind by more than a frame (a slow turn): pick up from now, never a burst.
        let next = due + Duration::from_millis(20);
        self.next_send = Some(if next < now { now + Duration::from_millis(20) } else { next });
    }
}

// ── The world: the forwarder, the relay's part, the desktop side, the page ───────────────────

/// A file the desktop side offered the browser, waiting for its dial.
struct Offered {
    offer: Offer,
    path: PathBuf,
}

struct World {
    brk: Break,
    hub: Hub,
    service: CallService,
    _forwarder: tokio::runtime::Runtime,
    seed: [u8; 32],
    desktop: String,
    browser: String,
    app: WebrtcHandle,
    call_with: Option<String>,
    voice: Voice,
    link: Option<FileLink>,
    early_ice: Vec<String>,
    link_events: Vec<LinkEvent>,
    offered: Option<Offered>,
    page: Vec<Value>,
    errors: Vec<String>,
    dir: PathBuf,
}

impl World {
    fn start(brk: Break) -> World {
        // The relay's forwarder, as `call_forwarder::start` runs it, on loopback: relayed
        // addresses are 127.0.0.1 and a port no socket holds (they are virtual).
        let forwarder = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().expect("a runtime for the forwarder");
        let service = CallService::default();
        let socket = forwarder.block_on(tokio::net::UdpSocket::bind("127.0.0.1:0")).expect("the forwarder on loopback");
        let port = socket.local_addr().unwrap().port();
        service.set_listening("127.0.0.1", port);
        forwarder.spawn(serve(socket, Forwarder::new(service.keys().clone(), Some(Ipv4Addr::LOCALHOST), port, Instant::now())));

        // The desktop's identity: a real Dilithium3 key from a seed, as the app derives it.
        let mut seed = [0u8; 32];
        getrandom::getrandom(&mut seed).expect("random bytes");
        let kp = humanity_protocol::pq_crypto::DilithiumKeypair::from_seed(&humanity_protocol::pq_crypto::derive_dilithium_seed(&seed));
        let desktop = hex::encode(kp.public_key());
        let app = WebrtcManager::start_on(desktop.clone(), String::new(), "127.0.0.1:0");

        let dir = std::env::temp_dir().join(format!("hum_browser_webrtc_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("downloads")).expect("a scratch folder");
        World {
            brk,
            hub: Hub::start(),
            service,
            _forwarder: forwarder,
            seed,
            desktop,
            browser: String::new(),
            app,
            call_with: None,
            voice: Voice::new(brk == Break::Silence),
            link: None,
            early_ice: Vec::new(),
            link_events: Vec::new(),
            offered: None,
            page: Vec::new(),
            errors: Vec::new(),
            dir,
        }
    }

    fn to_page(&self, msg: &RelayMessage) {
        let _ = self.hub.to_page.send(serde_json::to_string(msg).expect("a relay frame"));
    }

    fn order(&self, mut what: Value) {
        what["type"] = json!("harness_do");
        let _ = self.hub.to_page.send(what.to_string());
    }

    /// Everything that happened since the last turn, handled.
    fn pump(&mut self) {
        while let Ok(msg) = self.hub.from_page.try_recv() {
            self.from_page(msg);
        }
        for frame in self.app.poll_outbound() {
            self.from_desktop(&frame);
        }
        for ev in self.app.poll_events() {
            match ev {
                WebrtcEvent::VoiceConnected { .. } => {
                    self.voice.connected = true;
                    self.voice.next_send = Some(Instant::now());
                    say!("[desktop] the call's ICE connected; the tone starts");
                }
                WebrtcEvent::VoiceFrame { opus, .. } => self.voice.hear(&opus),
                WebrtcEvent::RelayReady { scope } => say!("[desktop] the forwarder gave this call its relayed address ({})", scope.label()),
                WebrtcEvent::RelayUnavailable { scope, lost } => say!("[desktop] RELAY UNAVAILABLE for {} (lost: {lost})", scope.label()),
                WebrtcEvent::Closed { .. } => say!("[desktop] the call's connection closed"),
                _ => {}
            }
        }
        let events = self.link.as_ref().map(|l| l.poll()).unwrap_or_default();
        for ev in events {
            if let LinkEvent::Signal(frame) = &ev {
                self.from_desktop(frame);
            } else {
                say!("[desktop] file link: {}", short_event(&ev));
            }
            self.link_events.push(ev);
        }
        if let Some(peer) = self.call_with.clone() {
            self.voice.tick(&self.app, &peer);
        }
    }

    /// Pump until `done` says so or `limit` runs out; whether it did.
    fn wait(&mut self, limit: Duration, done: impl Fn(&World) -> bool) -> bool {
        let until = Instant::now() + limit;
        while Instant::now() < until {
            self.pump();
            if done(self) {
                return true;
            }
            thread::sleep(Duration::from_millis(3));
        }
        false
    }

    /// The last report of `ty` from the page (with `tag`, when given).
    fn report(&self, ty: &str, tag: Option<&str>) -> Option<&Value> {
        self.page.iter().rev().find(|m| m["type"] == ty && tag.is_none_or(|t| m["tag"] == t))
    }

    // ── The relay's part: what the page sends on its chat socket ──

    fn from_page(&mut self, msg: Value) {
        let ty = msg["type"].as_str().unwrap_or("").to_string();
        match ty.as_str() {
            "harness_hello" => {
                self.browser = msg["key"].as_str().unwrap_or("").to_string();
                say!("[page] signed in as {}... ({})", &self.browser[..self.browser.len().min(12)], msg["ua"].as_str().unwrap_or(""));
            }
            "harness_error" => {
                let line = format!("{} {}", msg["message"].as_str().unwrap_or(""), msg["where"].as_str().unwrap_or(""));
                say!("[page] ERROR: {line}");
                self.errors.push(line);
            }
            t if t.starts_with("harness_") => self.page.push(msg),
            "voice_call" => self.page_voice_call(&msg),
            "call_credentials" => self.page_credentials(&msg),
            "webrtc_signal" => self.page_signal(&msg),
            "dm_put" => self.page_note(&msg),
            other => say!("[relay] the page sent {other:?}, which this check does not route"),
        }
    }

    /// The relay's credentials reply to `to` for the call with `other`, minted by its own keys.
    fn call_reply(&self, to: &str, other: &str, wrong_scope: bool) -> RelayMessage {
        let keys = self.service.keys();
        let scope = if wrong_scope { keys.call_scope(to, "not-this-call") } else { keys.call_scope(to, other) };
        let (username, credential) = keys.issue(&scope, unix_now());
        let creds = RelayCredentials {
            call: Some(other.to_string()),
            urls: self.service.urls().expect("the forwarder listens"),
            username,
            credential,
            ttl: CREDENTIAL_TTL_SECS,
            ..Default::default()
        };
        RelayMessage::CallCredentials { to: to.to_string(), creds }
    }

    /// The relay's credentials reply to `to` for transfer `id` with `peer` (a file scope, metered).
    fn file_reply(&self, to: &str, peer: &str, id: &str, wrong_scope: bool) -> RelayMessage {
        let keys = self.service.keys();
        let scope = if wrong_scope { keys.file_scope("00000000000000000000000000000000", to, peer) } else { keys.file_scope(id, to, peer) };
        let (username, credential) = keys.issue_metered(&scope, to, 0, unix_now());
        let creds = RelayCredentials {
            file: Some(id.to_string()),
            urls: self.service.urls().expect("the forwarder listens"),
            username,
            credential,
            ttl: CREDENTIAL_TTL_SECS,
            ..Default::default()
        };
        RelayMessage::CallCredentials { to: to.to_string(), creds }
    }

    /// The relay's check of a file pass: `issuer` gave `grantee` this pass on this server.
    fn pass_id(issuer: &str, grantee: &str, pass: &Value) -> Option<String> {
        humanity_protocol::pq_crypto::verify_file_pass(SERVER_DID, issuer, grantee, pass, unix_now()).ok().map(|p| p.id)
    }

    fn page_voice_call(&mut self, msg: &Value) {
        let action = msg["action"].as_str().unwrap_or("");
        say!("[relay] voice_call {action} from the page");
        match action {
            // The browser rings the desktop; the person at the desktop accepts (engine/call_relay.rs):
            // the accept goes back, and the app asks for this call's credentials.
            "ring" => {
                self.call_with = Some(self.browser.clone());
                self.to_page(&RelayMessage::VoiceCall {
                    from: self.desktop.clone(),
                    from_name: Some("Desktop app".into()),
                    to: self.browser.clone(),
                    action: "accept".into(),
                    friend_cert: None,
                });
                self.desktop_call_credentials();
            }
            // The page accepted the desktop's ring: the app asks for the credentials and offers
            // (engine/frame_ws_poll.rs, "accept").
            "accept" => {
                self.call_with = Some(self.browser.clone());
                self.desktop_call_credentials();
                self.app.offer_to_voice(self.browser.clone(), CALL_ROOM_ID.to_string());
            }
            "hangup" | "reject" => {
                self.app.close_peer(self.browser.clone());
                self.app.end_relay(CallScope::Call(self.browser.clone()));
                self.call_with = None;
                self.voice.next_send = None;
            }
            _ => {}
        }
    }

    /// The desktop asks for its call's credentials; the reply, read the app's way, goes to the
    /// WebRTC thread.
    fn desktop_call_credentials(&mut self) {
        let reply = serde_json::to_value(self.call_reply(&self.desktop, &self.browser, false)).unwrap();
        match call_relay::parse_reply(&reply) {
            Some(Reply::Granted(creds)) => self.app.use_relay(creds),
            other => panic!("the app could not read the relay's call credentials: {other:?}"),
        }
    }

    fn page_credentials(&mut self, msg: &Value) {
        let wrong = self.brk == Break::Forwarder;
        if let Some(call) = msg["call"].as_str() {
            self.to_page(&self.call_reply(&self.browser, call, wrong));
            say!("[relay] call credentials to the page{}", if wrong { " (for ANOTHER call: the red run)" } else { "" });
        } else if let Some(id) = msg["file"].as_str() {
            let peer = msg["peer"].as_str().unwrap_or("");
            let pass = &msg["file_pass"];
            let fits = [(peer, self.browser.as_str()), (self.browser.as_str(), peer)]
                .into_iter()
                .any(|(issuer, grantee)| Self::pass_id(issuer, grantee, pass).as_deref() == Some(id));
            if fits {
                self.to_page(&self.file_reply(&self.browser, peer, id, wrong));
                say!("[relay] file credentials to the page{}", if wrong { " (for ANOTHER transfer: the red run)" } else { "" });
            } else {
                say!("[relay] REFUSED the page's file credentials: its file pass does not check");
                let creds = RelayCredentials { file: Some(id.to_string()), refused: true, ..Default::default() };
                self.to_page(&RelayMessage::CallCredentials { to: self.browser.clone(), creds });
            }
        }
    }

    /// A `webrtc_signal` from the page: the relay forwards it to the desktop (with `file` on a dial
    /// its pass let in, and never the pass itself), and the desktop's engine routes it.
    fn page_signal(&mut self, msg: &Value) {
        let signal_type = msg["signal_type"].as_str().unwrap_or("").to_string();
        let file = if signal_type == "dc_offer" { Self::pass_id(&self.desktop, &self.browser, &msg["file_pass"]) } else { None };
        let mut data = msg["data"].clone();
        say!("[relay] {signal_type} page -> desktop{}", signal_note(&data));
        if self.brk == Break::Fingerprint && (signal_type.contains("offer") || signal_type.contains("answer")) {
            data = tamper_signal(&data);
            say!("[relay] {signal_type} to the desktop with its DTLS fingerprint CHANGED (the red run)");
        }
        let forwarded = RelayMessage::WebrtcSignal {
            from: self.browser.clone(),
            to: self.desktop.clone(),
            signal_type: signal_type.clone(),
            data,
            friend_cert: None,
            file_pass: None,
            file,
        };
        let frame = serde_json::to_value(&forwarded).unwrap();
        self.desktop_signal(&frame);
    }

    /// The desktop's engine with a forwarded `webrtc_signal`: a file connection's goes to the file
    /// link (engine/file_transfer.rs `on_signal`), a call's to the WebRTC thread under the call id,
    /// only from the person in the call (engine/frame_ws_poll.rs).
    fn desktop_signal(&mut self, frame: &Value) {
        let from = frame["from"].as_str().unwrap_or("").to_string();
        let signal_type = frame["signal_type"].as_str().unwrap_or("").to_string();
        let data = frame["data"].clone();
        match signal_type.as_str() {
            "dc_offer" => self.answer_dial(frame),
            "dc_answer" | "dc_ice" => match &self.link {
                Some(link) => link.send(Cmd::Signal { signal_type, data: signal_text(&data) }),
                None if signal_type == "dc_ice" => self.early_ice.push(signal_text(&data)),
                None => say!("[desktop] a {signal_type} with no file connection"),
            },
            "offer" | "answer" | "ice" if self.call_with.as_deref() == Some(from.as_str()) => {
                self.app.submit_voice_signal(from, CALL_ROOM_ID.to_string(), signal_type, data);
            }
            other => say!("[desktop] ignored a {other} signal"),
        }
    }

    /// A frame the desktop sends on its chat socket: the relay forwards a `webrtc_signal` to the
    /// page as itself, with `file` on a dial its pass let in.
    fn from_desktop(&mut self, frame: &str) {
        let Ok(v) = serde_json::from_str::<Value>(frame) else { return };
        if v["type"] != "webrtc_signal" {
            say!("[relay] the desktop sent {}, which this check does not route", v["type"]);
            return;
        }
        let signal_type = v["signal_type"].as_str().unwrap_or("").to_string();
        say!("[relay] {signal_type} desktop -> page{}", signal_note(&v["data"]));
        let file = if signal_type == "dc_offer" { Self::pass_id(&self.browser, &self.desktop, &v["file_pass"]) } else { None };
        self.to_page(&RelayMessage::WebrtcSignal {
            from: self.desktop.clone(),
            to: self.browser.clone(),
            signal_type,
            data: v["data"].clone(),
            friend_cert: None,
            file_pass: None,
            file,
        });
    }

    /// A sealed note from the page (stood in: its text): a file offer, which the desktop reads, checks
    /// and accepts, then dials for (the receiver dials, part 4).
    fn page_note(&mut self, msg: &Value) {
        if let Some(r) = msg["ref"].as_str() {
            let _ = self.hub.to_page.send(json!({ "type": "dm_put_ok", "ref": r }).to_string());
        }
        let text = msg["harness_note"].as_str().unwrap_or("");
        let Some(file_offer::Note::Offer(mut offer)) = file_offer::parse_note(text) else {
            say!("[desktop] the page's note {}", &text[..text.len().min(40)]);
            return;
        };
        if let Err(e) = file_offer::verify_file_pass(&self.browser, SERVER_DID, &self.desktop, &offer.pass, unix_now()) {
            self.errors.push(format!("the website's file pass did not check on the desktop: {e:?}"));
            return;
        }
        say!("[desktop] the website's offer of {} ({} bytes) read, its Dilithium3 pass checked; accepted", offer.name, offer.size);
        if self.brk == Break::Hash {
            offer.blake3 = blake3::hash(b"another file").to_hex().to_string();
            say!("[desktop] the offer's whole-file BLAKE3 CHANGED to another file's (the red run)");
        }
        let part = self.dir.join(format!("{}.part", offer.id));
        std::fs::OpenOptions::new().write(true).create(true).truncate(true).open(&part).unwrap().set_len(offer.size).unwrap();
        let proto = Receiving::new(offer.clone(), "the browser", true, Instant::now());
        let transfer = Transfer {
            id: offer.id.clone(),
            side: Side::Receive { proto, part, dir: self.dir.clone(), name: offer.name.clone(), sender: "the browser".into() },
        };
        let relay = self.desktop_file_relay(&offer.id);
        self.start_link(Setup {
            me: self.desktop.clone(),
            peer: self.browser.clone(),
            dial_pass: Some(offer.pass.clone()),
            their_offer: None,
            relay,
            transfers: vec![transfer],
            bind: "127.0.0.1:0".into(),
            early_ice: Vec::new(),
        });
    }

    /// The desktop's credentials for transfer `id`, read the engine's way.
    fn desktop_file_relay(&self, id: &str) -> crate::net::file_link::Relay {
        let reply = serde_json::to_value(self.file_reply(&self.desktop, &self.browser, id, false)).unwrap();
        match crate::engine::file_transfer::read_credentials(&reply) {
            Some((got, Some(relay))) if got == id => relay,
            other => panic!("the app could not read the relay's file credentials: {other:?}"),
        }
    }

    fn start_link(&mut self, setup: Setup) {
        self.link = None; // the last one's thread stops
        self.link_events.clear();
        self.link = Some(FileLink::start(setup));
    }

    /// The browser dials for the file the desktop offered: the desktop answers, sending it.
    fn answer_dial(&mut self, frame: &Value) {
        let Some(offered) = self.offered.as_ref() else {
            say!("[desktop] a dial for no offer");
            return;
        };
        if frame["file"].as_str() != Some(offered.offer.id.as_str()) {
            say!("[desktop] a dial that names no file of ours (file: {})", frame["file"]);
            return;
        }
        let (offer, path) = (offered.offer.clone(), offered.path.clone());
        let relay = self.desktop_file_relay(&offer.id);
        let transfer = Transfer { id: offer.id.clone(), side: Side::Send { proto: Sending::new(offer.clone(), None), path, receiver: "the browser".into() } };
        let early_ice = std::mem::take(&mut self.early_ice);
        self.start_link(Setup {
            me: self.desktop.clone(),
            peer: self.browser.clone(),
            dial_pass: None,
            their_offer: Some(signal_text(&frame["data"])),
            relay,
            transfers: vec![transfer],
            bind: "127.0.0.1:0".into(),
            early_ice,
        });
        say!("[desktop] answered the browser's dial for {}", offer.name);
    }

    // ── The shapes ──

    fn stats(&mut self, tag: &str) -> Option<Value> {
        self.order(json!({ "what": "stats", "tag": tag }));
        self.wait(Duration::from_secs(5), |w| w.report("harness_stats", Some(tag)).is_some());
        self.report("harness_stats", Some(tag)).cloned()
    }

    /// One call, `browser_dials` or the desktop dialling: Ok(the evidence) or Err(what failed).
    fn call(&mut self, browser_dials: bool) -> Result<String, String> {
        self.voice.reset();
        let who = if browser_dials { "browser-calls" } else { "desktop-calls" };
        if browser_dials {
            self.order(json!({ "what": "call", "key": self.desktop, "name": "Desktop app" }));
        } else {
            self.to_page(&RelayMessage::VoiceCall {
                from: self.desktop.clone(),
                from_name: Some("Desktop app".into()),
                to: self.browser.clone(),
                action: "ring".into(),
                friend_cert: None,
            });
            self.order(json!({ "what": "accept_call" }));
        }
        let up = self.wait(CALL_WAIT, |w| w.voice.connected);
        if up && self.brk == Break::Silence {
            self.order(json!({ "what": "mute" }));
        }
        if up {
            self.wait(CALL_WAIT, |w| w.voice.sent >= TONE_BEFORE_STATS);
        }
        let first = self.stats(&format!("{who}-1"));
        self.wait(Duration::from_millis(1500), |_| false);
        let second = self.stats(&format!("{who}-2"));
        self.order(json!({ "what": "hangup" }));
        self.wait(Duration::from_secs(1), |_| false);
        judge_call(up, &self.voice, first.as_ref(), second.as_ref())
    }

    /// The page offers a file; the desktop accepts, dials and receives it.
    fn file_to_desktop(&mut self) -> Result<String, String> {
        let (size, seed) = (PIECE_SIZE as usize * 2 + 175_000, 7u32);
        let name = "from-the-browser.bin";
        // What the last shape's file link said is not this one's.
        self.link = None;
        self.link_events.clear();
        self.early_ice.clear();
        self.order(json!({ "what": "offer_file", "key": self.desktop, "size": size, "seed": seed, "name": name }));
        let finished = self.wait(FILE_WAIT, |w| w.link_events.iter().any(|e| matches!(e, LinkEvent::Ended(..) | LinkEvent::Closed | LinkEvent::RelayUnavailable)));
        // The page's row, for the report.
        self.order(json!({ "what": "transfers", "tag": "to-desktop" }));
        self.wait(Duration::from_secs(3), |w| w.report("harness_transfers", Some("to-desktop")).is_some());
        let page_state = self
            .report("harness_transfers", Some("to-desktop"))
            .and_then(|r| r["transfers"].as_array().and_then(|a| a.iter().find(|t| t["role"] == "send").cloned()));
        let saved = self.link_events.iter().find_map(|e| match e {
            LinkEvent::Saved(_, path) => Some(path.clone()),
            _ => None,
        });
        let want = file_bytes(size, seed);
        let mut fails = Vec::new();
        if !finished {
            fails.push(format!("the transfer did not end within {} s (the desktop's link said: {})", FILE_WAIT.as_secs(), self.link_story()));
        }
        match &saved {
            Some(path) => match std::fs::read(path) {
                Ok(got) if got == want => {}
                Ok(got) => fails.push(format!("the desktop's file differs from the page's: {} bytes, BLAKE3 {} (the page's {})", got.len(), blake3::hash(&got).to_hex(), blake3::hash(&want).to_hex())),
                Err(e) => fails.push(format!("the desktop's saved file could not be read: {e}")),
            },
            None => fails.push(format!("the desktop saved no file (its link said: {})", self.link_story())),
        }
        if !self.link_events.iter().any(|e| matches!(e, LinkEvent::Ended(_, End::Done))) {
            fails.push("the desktop's side never said done".into());
        }
        let page_says = page_state.as_ref().map(|t| format!("{} ({})", t["state"].as_str().unwrap_or("?"), t["reason"].as_str().unwrap_or("-"))).unwrap_or_else(|| "no row".into());
        if fails.is_empty() {
            Ok(format!(
                "{size} bytes in {} pieces, the website's Sender to the app's Receiver; the desktop's file is the page's exactly (BLAKE3 {}); the page's row: {page_says}",
                file_offer::pieces_for(size as u64),
                blake3::hash(&want).to_hex()
            ))
        } else {
            fails.push(format!("the page's row: {page_says}"));
            fails.push(self.page_links("to-desktop"));
            Err(fails.join("; "))
        }
    }

    /// The desktop offers a file; the page accepts, dials, receives and saves it.
    fn file_to_browser(&mut self) -> Result<String, String> {
        let (size, seed) = (PIECE_SIZE as usize * 3 + 99_003, 11u32);
        let name = "to-the-browser.bin";
        // What the last shape's file link said is not this one's.
        self.link = None;
        self.link_events.clear();
        self.early_ice.clear();
        let bytes = file_bytes(size, seed);
        let path = self.dir.join("source.bin");
        std::fs::write(&path, &bytes).unwrap();
        let id = file_offer::new_id().expect("an id");
        let now = unix_now();
        let pass: FilePass = file_offer::mint_file_pass(&self.seed, SERVER_DID, &self.desktop, &self.browser, &id, now + 3600, now).expect("the app mints its pass");
        let hashes: Vec<[u8; 32]> = bytes.chunks(PIECE_SIZE as usize).map(|c| *blake3::hash(c).as_bytes()).collect();
        let mut offer = Offer {
            id: id.clone(),
            name: name.into(),
            size: size as u64,
            mime: "application/octet-stream".into(),
            blake3: blake3::hash(&bytes).to_hex().to_string(),
            piece: PIECE_SIZE,
            pieces: file_offer::pieces_for(size as u64),
            hashes: file_offer::hashes_digest(&hashes),
            pass,
        };
        if self.brk == Break::Hash {
            offer.blake3 = blake3::hash(b"another file").to_hex().to_string();
            say!("[desktop] the offer's whole-file BLAKE3 is ANOTHER file's (the red run)");
        }
        self.offered = Some(Offered { offer: offer.clone(), path });
        let inner = json!({ "text": offer.text(), "from": self.desktop, "to": self.browser, "ts": now * 1000 });
        self.order(json!({ "what": "note", "inner": inner }));
        let asked = self.wait(Duration::from_secs(10), |w| w.report("harness_noted", None).is_some());
        let row = self.report("harness_noted", None).and_then(|r| r["transfers"].as_array().and_then(|a| a.iter().find(|t| t["id"] == id.as_str()).cloned()));
        if !asked || row.as_ref().map(|t| t["state"] != "asked").unwrap_or(true) {
            return Err(format!("the page did not ask about the desktop's offer (its pass checked by the website's pq.js): row {row:?}"));
        }
        self.order(json!({ "what": "accept_file", "id": id }));
        let download = self.dir.join("downloads").join(name);
        let complete = |p: &Path| std::fs::metadata(p).map(|m| m.len() == size as u64).unwrap_or(false);
        let mut arrived = self.wait(FILE_WAIT, |w| complete(&download) || w.link_events.iter().any(|e| matches!(e, LinkEvent::Closed)));
        // The desktop's side can close a moment before the browser has written its download.
        if !complete(&download) {
            arrived = self.wait(Duration::from_secs(5), |_| complete(&download));
        }
        // A moment for the browser to finish with it, then the page's row.
        self.wait(Duration::from_millis(500), |_| false);
        self.order(json!({ "what": "transfers", "tag": "to-browser" }));
        self.wait(Duration::from_secs(3), |w| w.report("harness_transfers", Some("to-browser")).is_some());
        let page_row = self
            .report("harness_transfers", Some("to-browser"))
            .and_then(|r| r["transfers"].as_array().and_then(|a| a.iter().find(|t| t["id"] == id.as_str()).cloned()));
        let page_says = page_row.as_ref().map(|t| format!("{} ({})", t["state"].as_str().unwrap_or("?"), t["reason"].as_str().unwrap_or("-"))).unwrap_or_else(|| "no row".into());
        let mut fails = Vec::new();
        match std::fs::read(&download) {
            Ok(got) if got == bytes => {}
            Ok(got) => fails.push(format!("the browser's saved file differs: {} bytes, BLAKE3 {} (sent {})", got.len(), blake3::hash(&got).to_hex(), blake3::hash(&bytes).to_hex())),
            Err(_) => fails.push(format!("the browser saved no file{} (the desktop's link said: {})", if arrived { "" } else { " in time" }, self.link_story())),
        }
        if page_row.as_ref().map(|t| t["state"] != "saved").unwrap_or(true) {
            fails.push(format!("the page's row is not \"saved\": {page_says}"));
        }
        if !self.link_events.iter().any(|e| matches!(e, LinkEvent::Ended(_, End::Done))) {
            fails.push("the desktop's sending side never ended done".into());
        }
        if fails.is_empty() {
            Ok(format!(
                "{size} bytes in {} pieces, the app's Sender to the website's Receiver; the browser checked every piece and the whole file, then saved it, and the saved file is the desktop's exactly (BLAKE3 {}); the page's row: {page_says}",
                offer.pieces,
                blake3::hash(&bytes).to_hex()
            ))
        } else {
            fails.push(self.page_links("to-browser"));
            Err(fails.join("; "))
        }
    }

    /// What the page's file connections looked like at the report tagged `tag`, for the diagnosis.
    fn page_links(&self, tag: &str) -> String {
        let links = self.report("harness_transfers", Some(tag)).map(|r| r["links"].to_string()).unwrap_or_else(|| "no report".into());
        format!("the page's file connections: {links}")
    }

    /// What the desktop's file link reported, short.
    fn link_story(&self) -> String {
        let parts: Vec<String> = self.link_events.iter().filter(|e| !matches!(e, LinkEvent::Signal(_) | LinkEvent::Progress(..))).map(short_event).collect();
        if parts.is_empty() { "nothing".into() } else { parts.join(", ") }
    }
}

fn short_event(ev: &LinkEvent) -> String {
    match ev {
        LinkEvent::Signal(_) => "a signal".into(),
        LinkEvent::Progress(_, n) => format!("{n} pieces"),
        LinkEvent::Saved(_, p) => format!("saved {}", p.file_name().and_then(|n| n.to_str()).unwrap_or("?")),
        LinkEvent::Ended(_, end) => format!("ended {end:?}"),
        other => format!("{other:?}").split(['(', ' ']).next().unwrap_or("?").to_string(),
    }
}

/// The browser's statistics at one moment: the call's transport and its inbound audio.
fn stat<'a>(s: Option<&'a Value>, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(s?, |v, k| v.get(*k).filter(|x| !x.is_null()))
}

/// The verdict on one call: what the browser measured (two getStats a moment apart) and what the
/// desktop decoded.
fn judge_call(up: bool, voice: &Voice, first: Option<&Value>, second: Option<&Value>) -> Result<String, String> {
    let number_at = |s: Option<&Value>, path: &[&str]| stat(s, path).and_then(Value::as_f64).unwrap_or(0.0);
    let word_at = |s: Option<&Value>, path: &[&str]| stat(s, path).and_then(Value::as_str).unwrap_or("?").to_string();
    let mut fails = Vec::new();
    if !up {
        fails.push(format!("the desktop's ICE never connected within {} s", CALL_WAIT.as_secs()));
    }
    let dtls = word_at(second, &["call", "transport", "dtlsState"]);
    if dtls != "connected" {
        fails.push(format!("the browser's DTLS transport is {dtls:?}, not \"connected\""));
    }
    let (local, remote) = (word_at(second, &["call", "transport", "local", "type"]), word_at(second, &["call", "transport", "remote", "type"]));
    if local != "relay" || remote != "relay" {
        fails.push(format!("the browser's path is {local} to {remote}, not relay to relay"));
    }
    let (p1, p2) = (number_at(first, &["call", "inbound", "packetsReceived"]), number_at(second, &["call", "inbound", "packetsReceived"]));
    if !(p1 > 0.0 && p2 > p1) {
        fails.push(format!("the browser's inbound audio packets did not arrive and grow ({p1} then {p2})"));
    }
    let (e1, e2) = (number_at(first, &["call", "inbound", "totalAudioEnergy"]), number_at(second, &["call", "inbound", "totalAudioEnergy"]));
    if e2 - e1 < MIN_ENERGY_RISE {
        fails.push(format!("the browser heard no sound from the desktop: its audio energy went {e1:.4} to {e2:.4}, a rise under {MIN_ENERGY_RISE}"));
    }
    if voice.heard < 50 {
        fails.push(format!("the desktop decoded only {} audio frames from the browser", voice.heard));
    }
    if voice.loud < MIN_LOUD_FRAMES {
        fails.push(format!("what the desktop decoded from the browser was silence: {} of {} frames above {LOUD_RMS} RMS (the loudest {:.0})", voice.loud, voice.heard, voice.loudest));
    }
    if !fails.is_empty() {
        // What the browser saw of the connection, for the diagnosis.
        fails.push(format!(
            "the browser's connection: {} / ICE {} / gathering {}; candidates {}; pairs {}",
            word_at(second, &["call", "connectionState"]),
            word_at(second, &["call", "iceConnectionState"]),
            word_at(second, &["call", "iceGatheringState"]),
            stat(second, &["call", "candidates"]).map(|v| v.to_string()).unwrap_or_default(),
            stat(second, &["call", "pairs"]).map(|v| v.to_string()).unwrap_or_default()
        ));
    }
    if fails.is_empty() {
        Ok(format!(
            "the browser's DTLS {} ({} role, {}, {}; SRTP {}), path relay to relay; the browser received {} audio packets ({}), energy {:.4} to {:.4}, level {}; the desktop sent {} frames and decoded {} from the browser, {} of them sound (the loudest {:.0} RMS)",
            dtls,
            word_at(second, &["call", "transport", "dtlsRole"]),
            word_at(second, &["call", "transport", "tlsVersion"]),
            word_at(second, &["call", "transport", "dtlsCipher"]),
            word_at(second, &["call", "transport", "srtpCipher"]),
            p2,
            word_at(second, &["call", "inbound", "codec"]),
            e1,
            e2,
            number_at(second, &["call", "inbound", "audioLevel"]),
            voice.sent,
            voice.heard,
            voice.loud,
            voice.loudest
        ))
    } else {
        Err(fails.join("; "))
    }
}

/// The four shapes against a real browser. `just verify-browser-webrtc`.
#[test]
#[ignore = "needs node and an installed Chromium (Chrome or Edge): `just verify-browser-webrtc`"]
fn the_desktop_app_calls_and_sends_files_to_a_real_browser() {
    let started = *START.get_or_init(Instant::now);
    // The app's own log lines from its WebRTC and file link, shown with the rest (RUST_LOG to change).
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("humanity_engine::net::webrtc=info,humanity_engine::net::file_link=info"))
        .is_test(true)
        .try_init();
    let brk = Break::from_env();
    let mut world = World::start(brk);
    say!("[check] the page at {} (break: {brk:?})", world.hub.page_url());
    let browser = Browser::start(&world.hub.page_url(), &world.dir.join("downloads"));
    let hello = world.wait(Duration::from_secs(45), |w| !w.browser.is_empty());
    assert!(hello, "the page never signed in (no harness_hello within 45 s); errors: {:?}", world.errors);
    world.order(json!({ "what": "peer", "key": world.desktop, "server": SERVER_DID, "name": "Desktop app" }));
    assert!(world.wait(Duration::from_secs(10), |w| w.report("harness_ready", None).is_some()), "the page never said it was ready");

    let mut results: Vec<(&str, Result<String, String>)> = Vec::new();
    results.push(("1. the browser calls the desktop", world.call(true)));
    results.push(("2. the desktop calls the browser", world.call(false)));
    results.push(("3. a file from the browser to the desktop", world.file_to_desktop()));
    world.wait(Duration::from_secs(1), |_| false);
    results.push(("4. a file from the desktop to the browser", world.file_to_browser()));

    let errors = world.errors.clone();
    let dir = world.dir.clone();
    drop(browser);
    drop(world);
    let _ = std::fs::remove_dir_all(&dir);

    println!("\n== the desktop app against a real browser ({:.1} s) ==", started.elapsed().as_secs_f64());
    for (name, r) in &results {
        match r {
            Ok(evidence) => println!("PASS {name}: {evidence}"),
            Err(why) => println!("FAIL {name}: {why}"),
        }
    }
    if !errors.is_empty() {
        println!("errors in the page: {errors:?}");
    }
    let failed: Vec<String> = results.iter().filter_map(|(n, r)| r.as_ref().err().map(|e| format!("{n}: {e}"))).collect();
    assert!(failed.is_empty(), "the desktop app and the browser did not talk:\n{}", failed.join("\n"));
    assert!(errors.is_empty(), "the page reported errors: {errors:?}");
}

// THE RED RUNS (2026-11-07, Chrome 157, headless), each with HOS_BROWSER_WEBRTC_BREAK set, every one
// failing as it must, and every one green again without it:
//  fingerprint  all four shapes failed: the app refused each SDP ("remote sdp: remote fingerprint no
//               match", its log), the browser's DTLS stayed "connecting" with ICE connected, no
//               audio either way ("the browser's DTLS transport is \"connecting\", not
//               \"connected\""), and no file link opened.
//  forwarder    all four failed: the forwarder passed nothing between the two scopes, so no ICE
//               pair formed ("the desktop's ICE never connected within 30 s"; the page's own line
//               "Call disconnected."), and neither file link opened.
//  hash         the calls passed; both files failed on their whole-file BLAKE3: the app's receiver
//               "ended Failed(\"from-the-browser.bin did not match what the browser offered, so it
//               was not saved.\")", and the website's "the page's row is not \"saved\": failed
//               (mismatch)" with no download.
//  silence      the files passed; both calls failed on both ends: "the browser heard no sound from
//               the desktop: its audio energy went 0.0000 to 0.0000, a rise under 0.02" and "what
//               the desktop decoded from the browser was silence: 0 of 224 frames above 300 RMS".
//               This run first PASSED the browser's half: the judge then asked only that the energy
//               rise at all, and silence raised it by a hair. Hence MIN_ENERGY_RISE (the tone raises
//               it about 0.09), and MIN_LOUD_FRAMES where 5 had let 4 frames from before the mute
//               nearly pass.
// And the run that found BUG-274 (before the fix in chat-file-transfer.js): shape 4, straight after
// shape 3, "the browser saved no file in time ... the page's row is not \"saved\": receiving (-)",
// its second channel open on a connection the browser read "disconnected".
