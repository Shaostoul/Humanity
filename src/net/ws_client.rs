//! WebSocket client for the HumanityOS chat relay server.
//!
//! Connects to the relay at `/ws`, sends an `identify` message, and then
//! runs read/write pumps on a background thread. The game thread communicates
//! via `std::sync::mpsc` channels (non-blocking).

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// The link's honest lifecycle. The old model was a single `connected: bool`
/// initialized to TRUE "optimistically", which had two real consequences
/// during the 2026-08-13 outage: the reconnect backoff reset itself on every
/// spawn (so it retried forever at the minimum delay), and the history fetch
/// fired on a socket that never opened, stalling the render thread ~21 s per
/// cycle on the OS connect timeout: the operator's "app froze in chat"
/// report. Three states make the truth expressible: a fresh spawn is
/// CONNECTING (neither alive nor dead), only the network thread's
/// __CONNECTED__ sentinel proves Connected, and only a failure/close proves
/// Dropped.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LinkState {
    Connecting,
    Connected,
    Dropped,
}

/// A WebSocket client that talks to the relay server's chat protocol.
///
/// Communication with the game thread is entirely through channels:
/// - `send()` enqueues an outbound JSON string
/// - `poll_messages()` drains inbound JSON strings
pub struct WsClient {
    /// Send raw JSON strings to the network thread for transmission.
    sender: Option<mpsc::Sender<String>>,
    /// Receive raw JSON strings from the network thread.
    receiver: mpsc::Receiver<String>,
    /// Honest link lifecycle; see LinkState.
    state: LinkState,
    /// The relay server URL (e.g., "wss://united-humanity.us/ws").
    server_url: String,
    /// The user's display name (sent in the identify message).
    user_name: String,
    /// The user's public key hex (sent in the identify message).
    public_key: String,
}

impl WsClient {
    /// Create a new client and immediately connect on a background thread.
    ///
    /// `url` should be a WebSocket URL like `"wss://united-humanity.us/ws"`.
    /// `name` is the display name sent in the identify message.
    /// `pubkey_hex` is the Dilithium3 public key hex (the full-PQ identity).
    pub fn connect(url: &str, name: &str, pubkey_hex: &str) -> Self {
        Self::connect_with_kyber(url, name, pubkey_hex, "")
    }

    /// Connect with a Kyber768 public key (base64) for full-PQ E2E DMs.
    pub fn connect_with_kyber(url: &str, name: &str, pubkey_hex: &str, kyber_public_b64: &str) -> Self {
        let (tx_to_net, rx_from_game) = mpsc::channel::<String>();
        let (tx_to_game, rx_from_net) = mpsc::channel::<String>();

        let url_owned = url.to_string();
        let name_owned = name.to_string();
        let pubkey_owned = pubkey_hex.to_string();
        let kyber_owned = kyber_public_b64.to_string();

        thread::spawn(move || {
            run_connection(url_owned, name_owned, pubkey_owned, kyber_owned, rx_from_game, tx_to_game);
        });

        Self {
            sender: Some(tx_to_net),
            receiver: rx_from_net,
            state: LinkState::Connecting,
            server_url: url.to_string(),
            user_name: name.to_string(),
            public_key: pubkey_hex.to_string(),
        }
    }

    /// Send a raw JSON message string to the server.
    pub fn send(&self, msg: &str) {
        if let Some(ref tx) = self.sender {
            let _ = tx.send(msg.to_string());
        }
    }

    /// Non-blocking drain of all received JSON messages.
    pub fn poll_messages(&mut self) -> Vec<String> {
        let mut msgs = Vec::new();
        loop {
            match self.receiver.try_recv() {
                Ok(msg) => {
                    // A special disconnect sentinel
                    if msg == "__DISCONNECTED__" {
                        self.state = LinkState::Dropped;
                        continue;
                    }
                    // A special connected sentinel
                    if msg == "__CONNECTED__" {
                        self.state = LinkState::Connected;
                        continue;
                    }
                    msgs.push(msg);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    // Network thread gone entirely: dead either way.
                    self.state = LinkState::Dropped;
                    break;
                }
            }
        }
        msgs
    }

    /// TRUE only after the socket really opened (the __CONNECTED__ sentinel).
    /// A fresh spawn returns false here: gate work that needs a live relay
    /// (history fetch, backoff reset) on this, never on "the client exists".
    pub fn is_connected(&self) -> bool {
        self.state == LinkState::Connected
    }

    /// TRUE only when the link failed or closed. The teardown path gates on
    /// THIS, not on !is_connected(), so a still-handshaking client is not
    /// ripped down before it ever had a chance to open.
    pub fn is_dropped(&self) -> bool {
        self.state == LinkState::Dropped
    }

    /// Disconnect from the server.
    pub fn disconnect(&mut self) {
        self.sender = None;
        self.state = LinkState::Dropped;
    }

    /// The server URL this client is connected/connecting to.
    pub fn server_url(&self) -> &str {
        &self.server_url
    }

    /// The user name used for identification.
    pub fn user_name(&self) -> &str {
        &self.user_name
    }
}

/// Background thread: connect, identify, and run read/write pumps.
fn run_connection(
    url: String,
    name: String,
    pubkey: String,
    kyber_public: String,
    rx_from_game: mpsc::Receiver<String>,
    tx_to_game: mpsc::Sender<String>,
) {
    log::info!("WsClient: connecting to {}", url);
    crate::debug::push_debug(format!("WS connecting to {}", url));

    let connect_result = tungstenite::connect(&url);
    let (mut socket, _response) = match connect_result {
        Ok(pair) => pair,
        Err(e) => {
            log::error!("WsClient: connection failed: {}", e);
            crate::debug::push_debug(format!("WS connection FAILED: {}", e));
            let _ = tx_to_game.send("__DISCONNECTED__".to_string());
            return;
        }
    };

    log::info!("WsClient: connected to {}", url);
    crate::debug::push_debug(format!("WS connected to {}", url));
    let _ = tx_to_game.send("__CONNECTED__".to_string());

    // Send identify (matches the relay's RelayMessage::Identify).
    // Full-PQ: `public_key` IS the Dilithium3 hex; `kyber_public` is the
    // base64 Kyber768 encapsulation key the relay serves to DM senders.
    let identify = if kyber_public.is_empty() {
        serde_json::json!({
            "type": "identify",
            "public_key": pubkey,
            "display_name": name,
        })
    } else {
        serde_json::json!({
            "type": "identify",
            "public_key": pubkey,
            "display_name": name,
            "kyber_public": kyber_public,
        })
    };
    let identify_json = identify.to_string();
    crate::debug::push_debug(format!("WS >>> {}", identify_json));
    if let Err(e) = socket.send(tungstenite::Message::Text(identify_json)) {
        log::error!("WsClient: failed to send identify: {}", e);
        crate::debug::push_debug(format!("WS identify FAILED: {}", e));
        let _ = tx_to_game.send("__DISCONNECTED__".to_string());
        return;
    }

    // v0.200.0: ask for current server settings right after identifying
    // so the cached state is populated before the user opens the admin
    // page or sends a message that might exceed length limits. Anyone
    // can request — the relay broadcasts the response publicly.
    let req = serde_json::json!({ "type": "server_settings_request" }).to_string();
    crate::debug::push_debug(format!("WS >>> {}", req));
    if let Err(e) = socket.send(tungstenite::Message::Text(req)) {
        log::warn!("WsClient: failed to request server_settings: {} (will retry on next interaction)", e);
    }

    // Set the underlying TCP stream to non-blocking for the read/write loop
    set_nonblocking(&mut socket);

    loop {
        // ── Send outbound messages ──
        while let Ok(msg) = rx_from_game.try_recv() {
            match socket.send(tungstenite::Message::Text(msg)) {
                Ok(_) => {
                    // Flush to ensure TLS actually sends the data
                    match socket.flush() {
                        Ok(_) => {}
                        Err(tungstenite::Error::Io(ref e))
                            if e.kind() == std::io::ErrorKind::WouldBlock => {}
                        Err(e) => {
                            log::warn!("WsClient: flush error: {}", e);
                            let _ = tx_to_game.send("__DISCONNECTED__".to_string());
                            return;
                        }
                    }
                }
                Err(tungstenite::Error::Io(ref e))
                    if e.kind() == std::io::ErrorKind::WouldBlock =>
                {
                    // Socket busy, try again next iteration
                    log::debug!("WsClient: send WouldBlock, will retry");
                }
                Err(e) => {
                    log::warn!("WsClient: send error: {}", e);
                    let _ = tx_to_game.send("__DISCONNECTED__".to_string());
                    return;
                }
            }
        }

        // ── Receive inbound messages ──
        match socket.read() {
            Ok(tungstenite::Message::Text(text)) => {
                if tx_to_game.send(text).is_err() {
                    return; // game thread dropped the receiver
                }
            }
            Ok(tungstenite::Message::Close(_)) => {
                log::info!("WsClient: server closed connection");
                let _ = tx_to_game.send("__DISCONNECTED__".to_string());
                return;
            }
            Ok(tungstenite::Message::Ping(data)) => {
                let _ = socket.send(tungstenite::Message::Pong(data));
            }
            Err(tungstenite::Error::Io(ref e))
                if e.kind() == std::io::ErrorKind::WouldBlock =>
            {
                // No data yet, sleep briefly to avoid busy-spin
                thread::sleep(Duration::from_millis(5));
            }
            Err(tungstenite::Error::Protocol(
                tungstenite::error::ProtocolError::ResetWithoutClosingHandshake,
            )) => {
                log::warn!("WsClient: connection reset without close handshake");
                let _ = tx_to_game.send("__DISCONNECTED__".to_string());
                return;
            }
            Err(e) => {
                log::warn!("WsClient: read error (type: {:?}): {}", std::mem::discriminant(&e), e);
                let _ = tx_to_game.send("__DISCONNECTED__".to_string());
                return;
            }
            _ => {}
        }
    }
}

/// Set the underlying TCP stream to non-blocking mode.
fn set_nonblocking(
    socket: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
) {
    match socket.get_mut() {
        tungstenite::stream::MaybeTlsStream::Plain(s) => {
            let _ = s.set_nonblocking(true);
        }
        tungstenite::stream::MaybeTlsStream::NativeTls(tls_stream) => {
            let _ = tls_stream.get_mut().set_nonblocking(true);
        }
        other => {
            // Fallback: try to get the inner stream for any other TLS variant
            log::warn!("WsClient: unhandled TLS stream variant {:?}, non-blocking not set", std::mem::discriminant(other));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The regression that froze the app (2026-08-13): a fresh spawn used to
    /// report connected=true "optimistically", which reset the reconnect
    /// backoff every cycle AND green-lit a no-timeout history fetch on the
    /// render thread while the relay was dark. A listener that never accepts
    /// the WebSocket handshake holds the client in CONNECTING: it must be
    /// neither connected nor dropped.
    #[test]
    fn fresh_spawn_is_connecting_not_connected() {
        // Bind but never accept: TCP opens, the WS upgrade never answers.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        let mut c = WsClient::connect(&format!("ws://127.0.0.1:{port}/ws"), "t", "00");
        // Give the network thread a moment to spin up and start the dial.
        std::thread::sleep(Duration::from_millis(200));
        c.poll_messages();
        assert!(
            !c.is_connected(),
            "a client whose handshake never completed must not claim Connected"
        );
        assert!(
            !c.is_dropped(),
            "still handshaking is not Dropped either; teardown would strand it"
        );
        drop(listener);
    }

    /// A dead port must resolve to DROPPED (never Connected): connection
    /// refused is instant, so this also proves failures produce the sentinel
    /// the teardown path now gates on.
    #[test]
    fn refused_connect_becomes_dropped_never_connected() {
        // Grab a free port, then close the listener so the port is dead.
        let port = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
            l.local_addr().unwrap().port()
        };
        let mut c = WsClient::connect(&format!("ws://127.0.0.1:{port}/ws"), "t", "00");
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            c.poll_messages();
            assert!(!c.is_connected(), "refused connect must never read Connected");
            if c.is_dropped() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "refused connect did not resolve to Dropped within 5s"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}
