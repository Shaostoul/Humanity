//! Live video VIEWER (v0.857) — the receive side of the streaming pipeline.
//!
//! The mirror image of `net::live` (the publisher): it connects to the relay's
//! `/ws/live/sub/{stream}`, receives the MJPEG frames the publisher sent, decodes
//! each JPEG, and hands the newest decoded frame to the UI thread. This is what
//! lets you watch a stream INSIDE the native app instead of only on the web page.
//!
//! ## Threading
//!
//! ```text
//! network thread                         UI thread (egui)
//! --------------                         ----------------
//! WS recv -> JPEG decode -> latest slot   take_latest() -> upload texture -> paint
//! ```
//!
//! Only the NEWEST decoded frame is kept (a `Mutex<Option<DecodedFrame>>`, not a
//! queue). A viewer wants the present, not a backlog, and the UI paints at its own
//! rate, so an older undrawn frame is simply overwritten. Same drop-to-present
//! philosophy as the publisher.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// A decoded frame ready to become an egui texture: tightly-packed RGBA8.
pub struct DecodedFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// WHY a viewer's thread ended, as data rather than as English.
///
/// `status` below carries a sentence for a human, and that sentence is the
/// only thing a caller used to have. But a wall screen says something
/// different for "nobody is streaming under that name" than for "the socket
/// failed", and branching on the wording means a reworded sentence silently
/// changes what a screen decides. So the reason travels beside the words.
///
/// Stored as a `u8` in an atomic (there is no `AtomicEnum`); `from_u8` is the
/// only way back, and an unknown value reads as `Connection` rather than
/// pretending nothing went wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EndReason {
    /// Still running, or it ended without recording a reason.
    #[default]
    None,
    /// The relay answered "not live": nobody is publishing under that name.
    NotLive,
    /// The relay answered "at capacity": the stream is at its viewer ceiling.
    AtCapacity,
    /// The publisher stopped and the socket closed cleanly.
    Ended,
    /// The socket could not be opened, or it failed mid-stream.
    Connection,
}

impl EndReason {
    fn as_u8(self) -> u8 {
        match self {
            EndReason::None => 0,
            EndReason::NotLive => 1,
            EndReason::AtCapacity => 2,
            EndReason::Ended => 3,
            EndReason::Connection => 4,
        }
    }

    fn from_u8(v: u8) -> Self {
        match v {
            0 => EndReason::None,
            1 => EndReason::NotLive,
            2 => EndReason::AtCapacity,
            3 => EndReason::Ended,
            // Anything else can only be a bug on the storing side; treat it as
            // a fault rather than as health.
            _ => EndReason::Connection,
        }
    }
}

/// Shared viewer state, read by the UI each frame without blocking the network.
#[derive(Default)]
pub struct ViewerShared {
    pub connected: AtomicBool,
    pub frames: AtomicU64,
    /// The newest decoded frame, or None if none has arrived since the last take.
    pub latest: Mutex<Option<DecodedFrame>>,
    /// Non-empty if the stream is unavailable or ended.
    pub status: Mutex<String>,
    /// The machine-readable twin of `status` (see [`EndReason`]). Written in
    /// the same place, and only there.
    pub ended: AtomicU8,
}

/// A running viewer. Dropping it stops the network thread and frees the socket.
pub struct LiveViewer {
    stop: Arc<AtomicBool>,
    shared: Arc<ViewerShared>,
    stream_id: String,
}

impl LiveViewer {
    /// Connect to `stream_id` on `server` (a base URL like `https://host` or a
    /// `wss://host/ws` chat URL) and start decoding in the background.
    pub fn start(server: &str, stream_id: &str) -> Self {
        let ws_url = server
            .trim_end_matches('/')
            .trim_end_matches("/ws")
            .replace("https://", "wss://")
            .replace("http://", "ws://")
            + "/ws/live/sub/"
            + stream_id;

        let stop = Arc::new(AtomicBool::new(false));
        let shared = Arc::new(ViewerShared::default());

        let wstop = stop.clone();
        let wshared = shared.clone();
        thread::spawn(move || {
            viewer_thread(ws_url, wstop, wshared);
        });

        Self { stop, shared, stream_id: stream_id.to_string() }
    }

    pub fn shared(&self) -> &Arc<ViewerShared> {
        &self.shared
    }

    /// Take the newest decoded frame, if one arrived since the last call.
    pub fn take_latest(&self) -> Option<DecodedFrame> {
        self.shared.latest.lock().ok().and_then(|mut l| l.take())
    }

    pub fn is_connected(&self) -> bool {
        self.shared.connected.load(Ordering::Relaxed)
    }

    pub fn status(&self) -> String {
        self.shared.status.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Why this viewer's thread ended, for a caller that must say something
    /// different per case rather than echo [`LiveViewer::status`].
    pub fn end_reason(&self) -> EndReason {
        EndReason::from_u8(self.shared.ended.load(Ordering::Relaxed))
    }

    pub fn frames(&self) -> u64 {
        self.shared.frames.load(Ordering::Relaxed)
    }

    pub fn stream_id(&self) -> &str {
        &self.stream_id
    }
}

impl Drop for LiveViewer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn viewer_thread(ws_url: String, stop: Arc<AtomicBool>, shared: Arc<ViewerShared>) {
    let mut socket = match tungstenite::connect(&ws_url) {
        Ok((s, _)) => s,
        Err(e) => {
            *shared.status.lock().unwrap() = format!("Could not connect: {e}");
            shared.ended.store(EndReason::Connection.as_u8(), Ordering::Relaxed);
            return;
        }
    };

    // Non-blocking reads so the stop flag is honored promptly even with no frames.
    if let tungstenite::stream::MaybeTlsStream::Plain(s) = socket.get_ref() {
        let _ = s.set_read_timeout(Some(Duration::from_millis(200)));
    }
    #[cfg(feature = "native")]
    if let tungstenite::stream::MaybeTlsStream::NativeTls(s) = socket.get_ref() {
        let _ = s.get_ref().set_read_timeout(Some(Duration::from_millis(200)));
    }

    while !stop.load(Ordering::Relaxed) {
        match socket.read() {
            Ok(tungstenite::Message::Binary(bytes)) => {
                // [1B tag][8B PTS][jpeg]. We treat every non-config frame as an
                // image; MJPEG makes them all self-contained keyframes.
                if bytes.len() <= 9 {
                    continue;
                }
                let tag = bytes[0];
                if tag == 0 {
                    // Codec config (H.264 avcC). MJPEG never sends it; ignore.
                    continue;
                }
                let jpeg = &bytes[9..];
                if let Ok(img) = image::load_from_memory(jpeg) {
                    let rgba = img.to_rgba8();
                    let (w, h) = (rgba.width(), rgba.height());
                    shared.connected.store(true, Ordering::Relaxed);
                    shared.frames.fetch_add(1, Ordering::Relaxed);
                    // A picture arriving clears both halves of the fault: the
                    // words and the reason must never disagree.
                    *shared.status.lock().unwrap() = String::new();
                    shared.ended.store(EndReason::None.as_u8(), Ordering::Relaxed);
                    *shared.latest.lock().unwrap() =
                        Some(DecodedFrame { width: w, height: h, rgba: rgba.into_raw() });
                }
            }
            Ok(tungstenite::Message::Text(t)) => {
                // The relay tells us "not live" / "at capacity" as JSON text.
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&t) {
                    if v.get("ok").and_then(|o| o.as_bool()) == Some(false) {
                        let err = v.get("error").and_then(|e| e.as_str()).unwrap_or("unavailable");
                        // The relay's error CODE is the reason; the sentence
                        // beside it is only for a human to read.
                        let (reason, words) = match err {
                            "not live" => (EndReason::NotLive, "This stream is not live right now.".to_string()),
                            "at capacity" => (EndReason::AtCapacity, "This stream is at viewer capacity.".to_string()),
                            // An answer we do not have a case for is a
                            // protocol fault, not a healthy "nothing here".
                            other => (EndReason::Connection, other.to_string()),
                        };
                        *shared.status.lock().unwrap() = words;
                        shared.ended.store(reason.as_u8(), Ordering::Relaxed);
                        break;
                    }
                }
            }
            Ok(tungstenite::Message::Close(_)) => {
                if shared.status.lock().unwrap().is_empty() {
                    *shared.status.lock().unwrap() = "The stream ended.".to_string();
                    shared.ended.store(EndReason::Ended.as_u8(), Ordering::Relaxed);
                }
                break;
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                // No frame within the read timeout: loop so the stop flag is checked.
                continue;
            }
            Err(e) => {
                if shared.status.lock().unwrap().is_empty() {
                    *shared.status.lock().unwrap() = format!("Stream error: {e}");
                    shared.ended.store(EndReason::Connection.as_u8(), Ordering::Relaxed);
                }
                break;
            }
        }
    }

    shared.connected.store(false, Ordering::Relaxed);
    let _ = socket.close(None);
}
