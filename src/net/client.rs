//! WebSocket network client for multiplayer.
//!
//! Runs the WebSocket connection on a background thread.
//! Messages are passed to/from the game thread via channels.

use super::protocol::NetMessage;
use std::sync::mpsc;
use std::thread;

/// Connection state.
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

/// Network client managing a WebSocket connection.
pub struct NetClient {
    state: ConnectionState,
    /// Outbound messages (game thread -> network thread).
    tx_out: Option<mpsc::Sender<NetMessage>>,
    /// Inbound messages (network thread -> game thread).
    rx_in: mpsc::Receiver<NetMessage>,
    /// Sender for inbound (held by network thread).
    tx_in: mpsc::Sender<NetMessage>,
    /// Handle to the network thread.
    thread_handle: Option<thread::JoinHandle<()>>,
}

impl NetClient {
    pub fn new() -> Self {
        let (tx_in, rx_in) = mpsc::channel();
        Self {
            state: ConnectionState::Disconnected,
            tx_out: None,
            rx_in,
            tx_in,
            thread_handle: None,
        }
    }

    /// Connect to a WebSocket server URL (e.g., "ws://united-humanity.us/ws").
    pub fn connect(&mut self, server_url: &str) {
        if self.state == ConnectionState::Connected || self.state == ConnectionState::Connecting {
            return;
        }

        self.state = ConnectionState::Connecting;

        let (tx_out, rx_out) = mpsc::channel::<NetMessage>();
        self.tx_out = Some(tx_out);

        let tx_in = self.tx_in.clone();
        let url = server_url.to_string();

        self.thread_handle = Some(thread::spawn(move || {
            match tungstenite::connect(&url) {
                Ok((mut socket, _response)) => {
                    // Signal connected
                    let _ = tx_in.send(NetMessage::Pong { timestamp: 0.0 });

                    // Set non-blocking for read so we can also check outbound
                    let stream = socket.get_mut();
                    if let tungstenite::stream::MaybeTlsStream::Plain(ref s) = stream {
                        let _ = s.set_nonblocking(true);
                    }

                    loop {
                        // Send outbound messages
                        while let Ok(msg) = rx_out.try_recv() {
                            let json = match serde_json::to_string(&msg) {
                                Ok(j) => j,
                                Err(_) => continue,
                            };
                            if socket.send(tungstenite::Message::Text(json)).is_err() {
                                return;
                            }
                        }

                        // Receive inbound messages
                        match socket.read() {
                            Ok(tungstenite::Message::Text(text)) => {
                                if let Ok(msg) = serde_json::from_str::<NetMessage>(&text) {
                                    if tx_in.send(msg).is_err() {
                                        return;
                                    }
                                }
                            }
                            Ok(tungstenite::Message::Close(_)) => return,
                            Err(tungstenite::Error::Io(ref e))
                                if e.kind() == std::io::ErrorKind::WouldBlock =>
                            {
                                // No data available, sleep briefly
                                thread::sleep(std::time::Duration::from_millis(1));
                            }
                            Err(_) => return,
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    log::error!("WebSocket connection failed: {}", e);
                }
            }
        }));
    }

    /// Disconnect from the server.
    pub fn disconnect(&mut self) {
        self.tx_out = None;
        self.state = ConnectionState::Disconnected;
        // Thread will exit when tx_out is dropped (rx_out recv fails)
    }

    /// Send a message to the server.
    pub fn send(&self, msg: NetMessage) {
        if let Some(ref tx) = self.tx_out {
            let _ = tx.send(msg);
        }
    }

    /// Poll for received messages (non-blocking). Returns all pending messages.
    pub fn poll(&mut self) -> Vec<NetMessage> {
        let mut msgs = Vec::new();
        while let Ok(msg) = self.rx_in.try_recv() {
            // First message (Pong with timestamp 0) signals connection established
            if self.state == ConnectionState::Connecting {
                if let NetMessage::Pong { timestamp } = &msg {
                    if *timestamp == 0.0 {
                        self.state = ConnectionState::Connected;
                        continue;
                    }
                }
            }
            msgs.push(msg);
        }
        msgs
    }

    /// Current connection state.
    pub fn state(&self) -> &ConnectionState {
        &self.state
    }

    /// Whether the client is connected.
    pub fn is_connected(&self) -> bool {
        self.state == ConnectionState::Connected
    }
}
