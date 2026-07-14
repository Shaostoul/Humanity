//! Live video fanout (v0.853.0).
//!
//! A dedicated BINARY WebSocket path for live streaming, deliberately isolated
//! from the chat relay. It does NOT touch `relay.rs`, does NOT go through
//! `broadcast_tx` (that is a JSON enum re-serialized per socket, which would mean
//! base64-in-JSON video), and does NOT go through the chat WS (which is text-only
//! with a 128 KB cap behind a Fibonacci rate limiter).
//!
//! Topology: one publisher -> relay -> N viewers. This is a simple fanout, not an
//! SFU. VPS egress is the ceiling (bitrate x viewers); HLS is the answer when that
//! bites, not more WebRTC. See `docs/design/streaming.md`.
//!
//! ## Wire format
//!
//! Every frame is one binary WebSocket message:
//!
//! ```text
//! [1 byte tag][8 bytes PTS micros, big-endian][payload...]
//! ```
//!
//! | tag | meaning                                                        |
//! |-----|----------------------------------------------------------------|
//! | 0   | codec config (H.264 avcC / SPS+PPS). Cached, replayed to joiners |
//! | 1   | keyframe. Cached, replayed to joiners                            |
//! | 2   | delta frame                                                      |
//! | 3   | audio                                                            |
//!
//! The envelope is codec-agnostic on purpose: v1 ships MJPEG (every frame is a
//! keyframe, tag 1, no config), and the H.264 upgrade is a payload swap on the
//! same transport rather than a rewrite.
//!
//! ## The detail that makes or breaks it
//!
//! We cache the last codec-config and the last keyframe per stream and replay them
//! to each joining viewer immediately. Without that, a viewer who joins mid-GOP
//! stares at a black canvas until the next keyframe.
//!
//! ## Auth
//!
//! The publisher authenticates IN-BAND: the first WS message must be a JSON auth
//! frame. It is deliberately NOT a query string, because the Dilithium key (3904
//! hex chars) plus signature (~6600) makes a ~10 KB URL, which nginx rejects with
//! HTTP 414 (this exact bug bit the admin-stats route). A WS upgrade cannot carry
//! a body, so in-band is the only correct place for it.
//!
//! The stream id is the publisher's REGISTERED NAME, resolved server-side from the
//! signing key. You cannot publish to someone else's stream id, and no admin role
//! is required: anyone with a registered name can stream on their own name.

use axum::{
    extract::{Path, State, ws::{Message, WebSocket, WebSocketUpgrade}},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};
use tracing::{info, warn};

use crate::relay::relay::RelayState;

/// Largest single frame we will relay. A 720p MJPEG keyframe is ~100-200 KB;
/// an H.264 keyframe is far smaller. 2 MB is generous headroom and still bounds
/// the blast radius of a hostile/buggy publisher.
const MAX_FRAME_BYTES: usize = 2 * 1024 * 1024;

/// How long a publisher has to send its auth frame before we hang up.
const AUTH_TIMEOUT: Duration = Duration::from_secs(10);

/// Per-viewer queue depth. Live video is drop-tolerant: a viewer that falls
/// behind should SKIP to the present, not accumulate latency. `broadcast` gives
/// us exactly that via `RecvError::Lagged`.
const VIEWER_QUEUE: usize = 64;

/// One live stream, keyed by the publisher's registered name.
pub struct LiveStream {
    /// Byte fanout to viewers. Arc so a frame is cloned once, not per viewer.
    tx: broadcast::Sender<Arc<[u8]>>,
    /// Last codec-config frame (tag 0), replayed to joining viewers.
    config: std::sync::Mutex<Option<Arc<[u8]>>>,
    /// Last keyframe (tag 1), replayed to joining viewers so they see a picture
    /// immediately instead of waiting out the GOP.
    last_key: std::sync::Mutex<Option<Arc<[u8]>>>,
    /// Current viewer count.
    viewers: Arc<AtomicUsize>,
    /// Operator-supplied title.
    title: std::sync::Mutex<String>,
    /// When the publisher connected.
    started: Instant,
    /// Total frames relayed (diagnostics).
    frames: AtomicUsize,
}

/// Registry of live streams. Lives on `RelayState`.
#[derive(Default)]
pub struct LiveRegistry {
    streams: RwLock<HashMap<String, Arc<LiveStream>>>,
}

impl LiveRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    async fn get(&self, id: &str) -> Option<Arc<LiveStream>> {
        self.streams.read().await.get(id).cloned()
    }

    /// Public snapshot for the status API.
    pub async fn snapshot(&self) -> Vec<serde_json::Value> {
        self.streams
            .read()
            .await
            .iter()
            .map(|(id, s)| {
                serde_json::json!({
                    "id": id,
                    "title": s.title.lock().map(|t| t.clone()).unwrap_or_default(),
                    "viewers": s.viewers.load(Ordering::Relaxed),
                    "uptime_secs": s.started.elapsed().as_secs(),
                    "frames": s.frames.load(Ordering::Relaxed),
                })
            })
            .collect()
    }
}

/// The publisher's first WS message.
#[derive(Deserialize)]
struct AuthFrame {
    key: String,
    timestamp: u64,
    sig: String,
    #[serde(default)]
    title: String,
}

/// `GET /live/pub` - publisher socket. Stream id comes from the signing key's
/// registered name, so it is not in the path (nothing to spoof).
pub async fn pub_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<RelayState>>,
) -> impl IntoResponse {
    ws.max_message_size(MAX_FRAME_BYTES)
        .on_upgrade(move |socket| publisher_loop(socket, state))
}

/// `GET /live/sub/{stream}` - viewer socket. Unauthenticated by design: a live
/// stream is public.
pub async fn sub_handler(
    ws: WebSocketUpgrade,
    Path(stream): Path<String>,
    State(state): State<Arc<RelayState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| viewer_loop(socket, state, stream))
}

/// `GET /api/live` - what is live right now.
pub async fn list_handler(State(state): State<Arc<RelayState>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "streams": state.live.snapshot().await }))
}

/// Verify the publisher's auth frame and resolve its stream id (= registered name).
/// Returns None if the signature, freshness, or name lookup fails.
async fn authenticate(state: &Arc<RelayState>, auth: &AuthFrame) -> Option<String> {
    use crate::relay::handlers::broadcast::verify_dilithium_signature;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    if now_ms.saturating_sub(auth.timestamp) > 5 * 60 * 1000 {
        warn!("live: publisher auth rejected (stale timestamp)");
        return None;
    }

    // Dilithium verify is CPU-bound; keep it off the async executor.
    let (k, s, ts) = (auth.key.clone(), auth.sig.clone(), auth.timestamp);
    let ok = tokio::task::spawn_blocking(move || {
        verify_dilithium_signature(&k, "live_publish", ts, &s)
    })
    .await
    .unwrap_or(false);
    if !ok {
        warn!("live: publisher auth rejected (bad signature)");
        return None;
    }

    // Replay guard: the same signed auth frame cannot be reused to hijack the
    // stream inside the freshness window.
    if !state.auth_nonce_fresh(&auth.key, "live_publish", auth.timestamp) {
        warn!("live: publisher auth rejected (replay)");
        return None;
    }

    // The stream id IS the publisher's registered name. Resolved server-side, so
    // a publisher cannot claim someone else's id.
    match state.db.name_for_key(&auth.key) {
        Ok(Some(name)) => Some(name.to_lowercase()),
        _ => {
            warn!("live: publisher auth rejected (key has no registered name)");
            None
        }
    }
}

async fn publisher_loop(mut socket: WebSocket, state: Arc<RelayState>) {
    // --- Phase 1: in-band auth (see module docs for why not a query string).
    let first = match tokio::time::timeout(AUTH_TIMEOUT, socket.recv()).await {
        Ok(Some(Ok(Message::Text(t)))) => t,
        _ => {
            let _ = socket
                .send(Message::Text(r#"{"ok":false,"error":"expected auth frame"}"#.into()))
                .await;
            return;
        }
    };
    let Ok(auth) = serde_json::from_str::<AuthFrame>(&first) else {
        let _ = socket
            .send(Message::Text(r#"{"ok":false,"error":"malformed auth frame"}"#.into()))
            .await;
        return;
    };
    let Some(id) = authenticate(&state, &auth).await else {
        let _ = socket
            .send(Message::Text(r#"{"ok":false,"error":"unauthorized"}"#.into()))
            .await;
        return;
    };

    // --- Phase 2: claim the stream slot. One publisher per id; a second
    // connection for the same name replaces the first (reconnect after a crash
    // must not be locked out by its own zombie socket).
    let (tx, _) = broadcast::channel(VIEWER_QUEUE);
    let stream = Arc::new(LiveStream {
        tx,
        config: std::sync::Mutex::new(None),
        last_key: std::sync::Mutex::new(None),
        viewers: Arc::new(AtomicUsize::new(0)),
        title: std::sync::Mutex::new(if auth.title.is_empty() {
            format!("{id} is live")
        } else {
            auth.title.clone()
        }),
        started: Instant::now(),
        frames: AtomicUsize::new(0),
    });
    state.live.streams.write().await.insert(id.clone(), stream.clone());
    info!("live: '{id}' went live");

    let _ = socket
        .send(Message::Text(
            serde_json::json!({ "ok": true, "stream": id }).to_string().into(),
        ))
        .await;

    // --- Phase 3: pump frames.
    while let Some(Ok(msg)) = socket.recv().await {
        match msg {
            Message::Binary(bytes) => {
                if bytes.len() < 9 || bytes.len() > MAX_FRAME_BYTES {
                    continue; // malformed or oversized: drop, do not kill the stream
                }
                let frame: Arc<[u8]> = Arc::from(bytes.as_ref());
                match frame[0] {
                    0 => *stream.config.lock().unwrap() = Some(frame.clone()),
                    1 => *stream.last_key.lock().unwrap() = Some(frame.clone()),
                    _ => {}
                }
                stream.frames.fetch_add(1, Ordering::Relaxed);
                // Err just means "no viewers right now" - that is not a failure.
                let _ = stream.tx.send(frame);
            }
            // A text frame from a live publisher is a title update.
            Message::Text(t) => {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&t) {
                    if let Some(title) = v.get("title").and_then(|t| t.as_str()) {
                        *stream.title.lock().unwrap() = title.to_string();
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    // --- Phase 4: teardown. Only remove the entry if it is still OURS (a
    // reconnect may have replaced us, and we must not evict the new publisher).
    let mut streams = state.live.streams.write().await;
    if let Some(cur) = streams.get(&id) {
        if Arc::ptr_eq(cur, &stream) {
            streams.remove(&id);
            info!(
                "live: '{id}' ended after {}s, {} frames",
                stream.started.elapsed().as_secs(),
                stream.frames.load(Ordering::Relaxed)
            );
        }
    }
}

async fn viewer_loop(mut socket: WebSocket, state: Arc<RelayState>, id: String) {
    let id = id.to_lowercase();
    let Some(stream) = state.live.get(&id).await else {
        let _ = socket
            .send(Message::Text(r#"{"ok":false,"error":"not live"}"#.into()))
            .await;
        return;
    };

    let mut rx = stream.tx.subscribe();
    stream.viewers.fetch_add(1, Ordering::Relaxed);

    // Prime the viewer: codec config, then the last keyframe. THIS is what makes
    // the picture appear instantly instead of after a full GOP of black.
    let priming: Vec<Arc<[u8]>> = {
        let cfg = stream.config.lock().unwrap().clone();
        let key = stream.last_key.lock().unwrap().clone();
        cfg.into_iter().chain(key).collect()
    };
    let mut ok = true;
    for frame in priming {
        if socket.send(Message::Binary(frame.to_vec().into())).await.is_err() {
            ok = false;
            break;
        }
    }

    while ok {
        match rx.recv().await {
            Ok(frame) => {
                if socket.send(Message::Binary(frame.to_vec().into())).await.is_err() {
                    break;
                }
            }
            // This viewer fell behind. For live video the right answer is to skip
            // to the present, not to replay stale frames or disconnect them.
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }

    stream.viewers.fetch_sub(1, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The frame header is the contract between the app, the relay, and the web
    /// viewer. If this changes, all three change together.
    #[test]
    fn frame_header_is_tag_plus_be_micros() {
        let mut f = Vec::new();
        f.push(1u8); // keyframe
        f.extend_from_slice(&1_234_567u64.to_be_bytes());
        f.extend_from_slice(b"payload");

        assert_eq!(f[0], 1);
        let pts = u64::from_be_bytes(f[1..9].try_into().unwrap());
        assert_eq!(pts, 1_234_567);
        assert_eq!(&f[9..], b"payload");
        assert!(f.len() >= 9, "a valid frame is at least a header");
    }

    #[tokio::test]
    async fn registry_starts_empty_and_snapshots_cleanly() {
        let reg = LiveRegistry::new();
        assert!(reg.snapshot().await.is_empty());
        assert!(reg.get("nobody").await.is_none());
    }

    #[tokio::test]
    async fn a_joining_viewer_is_primed_with_config_then_keyframe() {
        let (tx, _) = broadcast::channel(8);
        let s = Arc::new(LiveStream {
            tx,
            config: std::sync::Mutex::new(Some(Arc::from(&[0u8, 0, 0, 0, 0, 0, 0, 0, 0, 9][..]))),
            last_key: std::sync::Mutex::new(Some(Arc::from(&[1u8, 0, 0, 0, 0, 0, 0, 0, 0, 7][..]))),
            viewers: Arc::new(AtomicUsize::new(0)),
            title: std::sync::Mutex::new("t".into()),
            started: Instant::now(),
            frames: AtomicUsize::new(0),
        });

        let priming: Vec<Arc<[u8]>> = {
            let cfg = s.config.lock().unwrap().clone();
            let key = s.last_key.lock().unwrap().clone();
            cfg.into_iter().chain(key).collect()
        };

        assert_eq!(priming.len(), 2, "config and keyframe both replayed");
        assert_eq!(priming[0][0], 0, "config goes first");
        assert_eq!(priming[1][0], 1, "then the keyframe");
    }
}
