//! WebSocket client for the HumanityOS chat relay server.
//!
//! Connects to the relay at `/ws`, sends an `identify` message, and then
//! runs read/write pumps on a background thread. The game thread communicates
//! via `std::sync::mpsc` channels (non-blocking).

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

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
    /// Whether the connection is alive.
    connected: bool,
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
    /// `pubkey_hex` is the Ed25519 public key hex string.
    pub fn connect(url: &str, name: &str, pubkey_hex: &str) -> Self {
        let (tx_to_net, rx_from_game) = mpsc::channel::<String>();
        let (tx_to_game, rx_from_net) = mpsc::channel::<String>();

        let url_owned = url.to_string();
        let name_owned = name.to_string();
        let pubkey_owned = pubkey_hex.to_string();

        thread::spawn(move || {
            run_connection(url_owned, name_owned, pubkey_owned, rx_from_game, tx_to_game);
        });

        Self {
            sender: Some(tx_to_net),
            receiver: rx_from_net,
            connected: true, // optimistic; we'll detect disconnection on poll
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
                        self.connected = false;
                        continue;
                    }
                    // A special connected sentinel
                    if msg == "__CONNECTED__" {
                        self.connected = true;
                        continue;
                    }
                    msgs.push(msg);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.connected = false;
                    break;
                }
            }
        }
        msgs
    }

    /// Whether the WebSocket connection is believed to be alive.
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Disconnect from the server.
    pub fn disconnect(&mut self) {
        self.sender = None;
        self.connected = false;
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
    rx_from_game: mpsc::Receiver<String>,
    tx_to_game: mpsc::Sender<String>,
) {
    log::info!("WsClient: connecting to {}", url);

    let connect_result = tungstenite::connect(&url);
    let (mut socket, _response) = match connect_result {
        Ok(pair) => pair,
        Err(e) => {
            log::error!("WsClient: connection failed: {}", e);
            let _ = tx_to_game.send("__DISCONNECTED__".to_string());
            return;
        }
    };

    log::info!("WsClient: connected to {}", url);
    let _ = tx_to_game.send("__CONNECTED__".to_string());

    // Send identify message (matches the relay's RelayMessage::Identify)
    let identify = serde_json::json!({
        "type": "identify",
        "public_key": pubkey,
        "display_name": name,
    });
    if let Err(e) = socket.send(tungstenite::Message::Text(identify.to_string())) {
        log::error!("WsClient: failed to send identify: {}", e);
        let _ = tx_to_game.send("__DISCONNECTED__".to_string());
        return;
    }

    // Set the underlying TCP stream to non-blocking for the read/write loop
    set_nonblocking(&mut socket);

    loop {
        // ── Send outbound messages ──
        while let Ok(msg) = rx_from_game.try_recv() {
            if socket.send(tungstenite::Message::Text(msg)).is_err() {
                let _ = tx_to_game.send("__DISCONNECTED__".to_string());
                return;
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
            Err(e) => {
                log::warn!("WsClient: read error: {}", e);
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
        _ => {
            log::warn!("WsClient: could not set non-blocking mode on TLS stream variant");
        }
    }
}
