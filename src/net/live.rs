//! Live video publisher (v0.853.0) — the thing that makes Studio's "Go Live" real.
//!
//! Frames come off the GPU on the render thread, get downscaled + JPEG-encoded on a
//! worker thread, and go out over a binary WebSocket to the relay's `/live/pub`,
//! which fans them out to viewers. See `docs/design/streaming.md` for why MJPEG and
//! not H.264 (short version: there is no pure-Rust real-time video encoder, and every
//! C-toolchain one is a dependency class this project has twice refused; the wire
//! format is codec-agnostic so H.264 is a payload swap, not a rewrite).
//!
//! ## Threading (this is the load-bearing part)
//!
//! The render thread must NEVER block on encoding. So:
//!
//! ```text
//! render thread          worker thread                    relay
//! ------------           -------------                    -----
//! submit_frame(raw)  ->  [bounded chan, cap 1]
//!                        downscale -> JPEG -> envelope  -> WS binary
//! ```
//!
//! The channel holds ONE frame. If the worker is still busy when the next frame
//! arrives, the new frame is DROPPED. That is the correct behavior for live video:
//! a viewer wants the present, not a backlog. Dropped frames show up honestly in
//! `LiveStats::dropped` rather than as creeping latency.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

/// Frame tags. Must match `src/relay/live.rs` and `web/pages/live.html`.
const TAG_KEYFRAME: u8 = 1;

/// A raw frame handed over by the render thread. Pixels are still in the GPU's
/// layout (padded rows, possibly BGRA) because converting them is the worker's job,
/// not the render thread's.
pub struct RawFrame {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub bytes_per_row: u32,
    pub bgra: bool,
}

/// What the operator configured in Studio.
#[derive(Clone)]
pub struct LiveConfig {
    /// Relay base URL, e.g. `https://united-humanity.us`.
    pub server: String,
    pub title: String,
    /// Target output height (720 = 720p). Width follows the source aspect ratio.
    pub target_height: u32,
    /// JPEG quality, 1-100. 70 is a reasonable live default.
    pub quality: u8,
    /// Frames per second to actually publish. The render loop runs much faster;
    /// we sample it.
    pub fps: u32,
}

impl LiveConfig {
    /// Parse the target height out of a "WIDTHxHEIGHT" picker string, clamped to a
    /// sane MJPEG band. Below 240 is pointless; above 1080 the bandwidth explodes for
    /// a hobby stream. The downscale never upscales, so a smaller window caps it further.
    /// A garbage string falls back to 720. Shared with lib.rs so the go-live path and
    /// its test agree.
    pub fn height_from_resolution(res: &str) -> u32 {
        res.split(['x', 'X'])
            .nth(1)
            .and_then(|h| h.trim().parse::<u32>().ok())
            .unwrap_or(720)
            .clamp(240, 1080)
    }
}

impl Default for LiveConfig {
    fn default() -> Self {
        Self {
            server: "https://united-humanity.us".into(),
            title: String::new(),
            target_height: 720,
            quality: 70,
            fps: 15,
        }
    }
}

/// Live counters, read by the Studio page every frame. Atomics so the UI never
/// blocks on the worker.
#[derive(Default)]
pub struct LiveStats {
    pub connected: AtomicBool,
    pub sent: AtomicU64,
    pub dropped: AtomicU64,
    /// Bytes sent, for a real bitrate readout.
    pub bytes: AtomicU64,
    pub viewers: AtomicU32,
    /// Last error, if the worker died. Empty means none.
    pub error: std::sync::Mutex<String>,
    /// The stream id the relay assigned us (our registered name). Empty until the
    /// relay accepts the stream; this is what the public watch URL is built from.
    pub stream_id: std::sync::Mutex<String>,
}

/// A running broadcast. Dropping it stops the stream.
pub struct LivePublisher {
    tx: mpsc::SyncSender<RawFrame>,
    stop: Arc<AtomicBool>,
    stats: Arc<LiveStats>,
    cfg: LiveConfig,
    /// When we last accepted a frame, for FPS sampling.
    last_accept: Instant,
    started: Instant,
}

impl LivePublisher {
    /// Start publishing. Returns immediately; the connection happens on the worker.
    /// `seed32` is the BIP39 seed — the same identity the chat signs with.
    pub fn start(cfg: LiveConfig, seed32: &[u8]) -> Self {
        // Capacity 1: hold at most one in-flight frame, drop the rest. See module docs.
        let (tx, rx) = mpsc::sync_channel::<RawFrame>(1);
        let stop = Arc::new(AtomicBool::new(false));
        let stats = Arc::new(LiveStats::default());

        let seed = seed32.to_vec();
        let wcfg = cfg.clone();
        let wstop = stop.clone();
        let wstats = stats.clone();
        thread::spawn(move || {
            if let Err(e) = worker(wcfg, seed, rx, wstop.clone(), wstats.clone()) {
                *wstats.error.lock().unwrap() = e;
            }
            wstats.connected.store(false, Ordering::Relaxed);
        });

        Self {
            tx,
            stop,
            stats,
            cfg,
            last_accept: Instant::now() - Duration::from_secs(1),
            started: Instant::now(),
        }
    }

    /// Hand a captured frame to the encoder. Called from the render thread — this
    /// NEVER blocks and never allocates beyond the move.
    pub fn submit_frame(&mut self, frame: RawFrame) {
        // Sample down to the configured fps; the render loop runs far faster.
        let interval = Duration::from_secs_f32(1.0 / self.cfg.fps.max(1) as f32);
        if self.last_accept.elapsed() < interval {
            return;
        }
        self.last_accept = Instant::now();

        match self.tx.try_send(frame) {
            Ok(()) => {}
            // Worker still busy: drop this frame rather than queue latency.
            Err(mpsc::TrySendError::Full(_)) => {
                self.stats.dropped.fetch_add(1, Ordering::Relaxed);
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {}
        }
    }

    /// Does the encoder want a frame right now? Lets the renderer skip the readback
    /// entirely (which is the expensive part) on frames we would only drop.
    pub fn wants_frame(&self) -> bool {
        let interval = Duration::from_secs_f32(1.0 / self.cfg.fps.max(1) as f32);
        self.last_accept.elapsed() >= interval
    }

    pub fn stats(&self) -> &Arc<LiveStats> {
        &self.stats
    }

    pub fn uptime(&self) -> Duration {
        self.started.elapsed()
    }

    /// Average kilobits/sec since the stream started.
    pub fn kbps(&self) -> u32 {
        let secs = self.started.elapsed().as_secs_f64().max(1.0);
        let bits = self.stats.bytes.load(Ordering::Relaxed) as f64 * 8.0;
        (bits / secs / 1000.0) as u32
    }
}

impl Drop for LivePublisher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Downscale + convert to RGB8 in one pass.
///
/// Box-averages every source pixel that lands on a destination pixel, which is both
/// cheap (one pass over the source) and much better looking than nearest-neighbour
/// when going 1080p -> 720p. Also un-swizzles BGRA (what a DX12 swapchain hands us)
/// and drops the padding wgpu adds to each row (`bytes_per_row` is 256-aligned).
fn downscale_to_rgb(f: &RawFrame, dst_w: u32, dst_h: u32) -> Vec<u8> {
    let (sw, sh) = (f.width as usize, f.height as usize);
    let (dw, dh) = (dst_w.max(1) as usize, dst_h.max(1) as usize);
    let bpr = f.bytes_per_row as usize;

    // Accumulate into u32 sums so we can average without overflow.
    let mut acc = vec![0u32; dw * dh * 3];
    let mut count = vec![0u32; dw * dh];

    for sy in 0..sh {
        let dy = sy * dh / sh;
        let row = &f.pixels[sy * bpr..sy * bpr + sw * 4];
        for sx in 0..sw {
            let dx = sx * dw / sw;
            let px = &row[sx * 4..sx * 4 + 4];
            let (r, g, b) = if f.bgra {
                (px[2], px[1], px[0])
            } else {
                (px[0], px[1], px[2])
            };
            let i = dy * dw + dx;
            acc[i * 3] += r as u32;
            acc[i * 3 + 1] += g as u32;
            acc[i * 3 + 2] += b as u32;
            count[i] += 1;
        }
    }

    let mut out = vec![0u8; dw * dh * 3];
    for i in 0..dw * dh {
        let n = count[i].max(1);
        out[i * 3] = (acc[i * 3] / n) as u8;
        out[i * 3 + 1] = (acc[i * 3 + 1] / n) as u8;
        out[i * 3 + 2] = (acc[i * 3 + 2] / n) as u8;
    }
    out
}

/// Wrap a payload in the wire envelope: `[1B tag][8B PTS micros BE][payload]`.
fn envelope(tag: u8, pts_micros: u64, payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(9 + payload.len());
    v.push(tag);
    v.extend_from_slice(&pts_micros.to_be_bytes());
    v.extend_from_slice(payload);
    v
}

fn worker(
    cfg: LiveConfig,
    seed: Vec<u8>,
    rx: mpsc::Receiver<RawFrame>,
    stop: Arc<AtomicBool>,
    stats: Arc<LiveStats>,
) -> Result<(), String> {
    // --- Connect.
    let ws_url = cfg
        .server
        .replace("https://", "wss://")
        .replace("http://", "ws://")
        .trim_end_matches('/')
        .to_string()
        + "/ws/live/pub";
    let (mut socket, _) =
        tungstenite::connect(&ws_url).map_err(|e| format!("could not reach {ws_url}: {e}"))?;

    // --- Authenticate in-band (see the relay module docs: a Dilithium key + sig in a
    // query string is a ~10 KB URL and nginx 414s it).
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let pq = crate::net::identity::derive_pq_identity(&seed)
        .map_err(|e| format!("could not derive identity: {e}"))?;
    let sig = crate::net::identity::pq_sign_chat(&seed, "live_publish", ts);
    let auth = serde_json::json!({
        "key": pq.dilithium_hex,
        "timestamp": ts,
        "sig": sig,
        "title": cfg.title,
    });
    socket
        .send(tungstenite::Message::Text(auth.to_string().into()))
        .map_err(|e| format!("auth send failed: {e}"))?;

    // The relay answers with {"ok":true,"stream":"<name>"} or an error.
    let reply = socket
        .read()
        .map_err(|e| format!("no auth reply: {e}"))?
        .into_text()
        .map_err(|e| format!("bad auth reply: {e}"))?;
    let v: serde_json::Value =
        serde_json::from_str(&reply).map_err(|e| format!("bad auth reply: {e}"))?;
    if !v.get("ok").and_then(|o| o.as_bool()).unwrap_or(false) {
        let err = v.get("error").and_then(|e| e.as_str()).unwrap_or("rejected");
        return Err(format!("relay refused the stream: {err}"));
    }
    let stream_id = v
        .get("stream")
        .and_then(|s| s.as_str())
        .unwrap_or_default()
        .to_string();
    *stats.stream_id.lock().unwrap() = stream_id.clone();
    stats.connected.store(true, Ordering::Relaxed);

    let start = Instant::now();

    // Viewer count runs on ITS OWN thread. It used to be inline in the pump loop
    // below, where a slow /api/live (VPS under load) blocked the frame drain for up
    // to the 2s ureq timeout, stalling video and dropping every frame in that window.
    // ureq is a blocking client, so the only correct place for it is off the hot path.
    {
        let poll_stop = stop.clone();
        let poll_stats = stats.clone();
        let poll_server = cfg.server.clone();
        let poll_id = stream_id.clone();
        if !poll_id.is_empty() {
            thread::spawn(move || {
                let url = format!("{}/api/live", poll_server.trim_end_matches('/'));
                while !poll_stop.load(Ordering::Relaxed) {
                    if let Ok(resp) = ureq::get(&url).timeout(Duration::from_secs(2)).call() {
                        if let Ok(body) = resp.into_string() {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                                let n = v["streams"]
                                    .as_array()
                                    .and_then(|a| a.iter().find(|s| s["id"] == poll_id.as_str()))
                                    .and_then(|s| s["viewers"].as_u64())
                                    .unwrap_or(0);
                                poll_stats.viewers.store(n as u32, Ordering::Relaxed);
                            }
                        }
                    }
                    // Sleep in small slices so Stop is honored within ~200ms rather
                    // than after a full 3s.
                    for _ in 0..15 {
                        if poll_stop.load(Ordering::Relaxed) {
                            break;
                        }
                        thread::sleep(Duration::from_millis(200));
                    }
                }
            });
        }
    }

    // --- Pump.
    while !stop.load(Ordering::Relaxed) {
        let frame = match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(f) => f,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // No frame this window (page hidden, game paused). Keep the socket
                // warm so the stream does not drop out from under the operator.
                let _ = socket.send(tungstenite::Message::Ping(Vec::new().into()));
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };

        // Preserve aspect ratio; only ever downscale.
        let (sw, sh) = (frame.width, frame.height);
        let dh = cfg.target_height.min(sh).max(2);
        let dw = ((sw as f32 * dh as f32 / sh as f32) as u32).max(2) & !1; // keep it even

        let rgb = downscale_to_rgb(&frame, dw, dh);

        let mut jpeg = Vec::with_capacity(rgb.len() / 8);
        {
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(
                &mut jpeg,
                cfg.quality.clamp(1, 100),
            );
            enc.encode(&rgb, dw, dh, image::ExtendedColorType::Rgb8)
                .map_err(|e| format!("jpeg encode failed: {e}"))?;
        }

        // MJPEG: every frame stands alone, so every frame is a keyframe. That is
        // exactly what the relay's join-priming cache wants.
        let pts = start.elapsed().as_micros() as u64;
        let msg = envelope(TAG_KEYFRAME, pts, &jpeg);
        let n = msg.len() as u64;
        if let Err(e) = socket.send(tungstenite::Message::Binary(msg.into())) {
            return Err(format!("stream dropped: {e}"));
        }
        stats.sent.fetch_add(1, Ordering::Relaxed);
        stats.bytes.fetch_add(n, Ordering::Relaxed);
    }

    let _ = socket.close(None);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The envelope is the contract shared by the app, the relay, and the web viewer.
    #[test]
    fn envelope_is_tag_then_be_pts_then_payload() {
        let e = envelope(TAG_KEYFRAME, 42, b"jpeg");
        assert_eq!(e[0], TAG_KEYFRAME);
        assert_eq!(u64::from_be_bytes(e[1..9].try_into().unwrap()), 42);
        assert_eq!(&e[9..], b"jpeg");
    }

    /// A DX12 swapchain hands us BGRA with 256-aligned row padding. Both must be
    /// handled or the stream comes out blue and skewed.
    #[test]
    fn downscale_unswizzles_bgra_and_strips_row_padding() {
        // 2x2 solid red in BGRA, with a padded row stride.
        let bpr = 16; // 2 px * 4 bytes = 8, padded to 16
        let mut pixels = vec![0u8; bpr * 2];
        for y in 0..2 {
            for x in 0..2 {
                let i = y * bpr + x * 4;
                pixels[i] = 0; // B
                pixels[i + 1] = 0; // G
                pixels[i + 2] = 255; // R
                pixels[i + 3] = 255; // A
            }
        }
        let f = RawFrame { pixels, width: 2, height: 2, bytes_per_row: bpr as u32, bgra: true };

        let rgb = downscale_to_rgb(&f, 2, 2);
        assert_eq!(rgb.len(), 2 * 2 * 3);
        for px in rgb.chunks_exact(3) {
            assert_eq!(px, [255, 0, 0], "BGRA red must come out as RGB red");
        }
    }

    /// Downscaling averages rather than dropping pixels: a half-black/half-white
    /// source collapsed to 1x1 must be grey, not one or the other.
    #[test]
    fn downscale_box_averages() {
        let bpr = 8;
        let mut pixels = vec![0u8; bpr * 2];
        // Row 0 white, row 1 black (RGBA).
        for x in 0..2 {
            let i = x * 4;
            pixels[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
        }
        let f = RawFrame { pixels, width: 2, height: 2, bytes_per_row: bpr as u32, bgra: false };

        let rgb = downscale_to_rgb(&f, 1, 1);
        assert_eq!(rgb.len(), 3);
        assert!(
            (120..=135).contains(&rgb[0]),
            "half white + half black should average to mid grey, got {}",
            rgb[0]
        );
    }

    /// THE end-to-end test: a real `LivePublisher` (real identity, real JPEG encoder,
    /// real WebSocket) against a real relay, with a real viewer on the other side that
    /// DECODES what arrives and checks the pixels.
    ///
    /// Unit-testing the envelope and the downscale separately proves neither of them
    /// is wired to anything. This proves the whole chain: a frame in the GPU's exact
    /// layout (BGRA, 256-padded rows) goes in one end, and a correctly-sized,
    /// correctly-coloured, decodable image comes out the other.
    ///
    /// Native-only because that is where the publisher lives; the `native` feature
    /// includes `relay`, so both halves are available here.
    #[tokio::test]
    async fn a_real_frame_survives_the_whole_pipeline_to_a_viewer() {
        use axum::routing::get;
        use futures::StreamExt;

        // --- Relay with our test identity's name registered.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path =
            std::env::temp_dir().join(format!("hum_live_e2e_{}_{nanos}.db", std::process::id()));
        let db = crate::relay::storage::Storage::open(&path).expect("open test db");

        let seed = [7u8; 32];
        let pq = crate::net::identity::derive_pq_identity(&seed).expect("derive identity");
        db.register_name("streamer", &pq.dilithium_hex).expect("register name");

        let state = Arc::new(crate::relay::relay::RelayState::new(db));
        let app = axum::Router::new()
            .route("/ws/live/pub", get(crate::relay::live::pub_handler))
            .route("/ws/live/sub/{stream}", get(crate::relay::live::sub_handler))
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        // --- Start the real publisher.
        let cfg = LiveConfig {
            server: format!("http://127.0.0.1:{port}"),
            title: "E2E".into(),
            target_height: 36, // tiny, so the test is fast
            quality: 90,
            fps: 30,
        };
        let mut publisher = LivePublisher::start(cfg, &seed);

        // Let the worker connect and authenticate before a viewer subscribes.
        tokio::time::sleep(Duration::from_millis(400)).await;
        assert!(
            publisher.stats().connected.load(Ordering::Relaxed),
            "publisher failed to authenticate: {}",
            publisher.stats().error.lock().unwrap()
        );

        let (mut viewer, _) =
            tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws/live/sub/streamer"))
                .await
                .expect("viewer connects");
        tokio::time::sleep(Duration::from_millis(150)).await;

        // --- Feed it a frame in the GPU's real layout: BGRA, 256-aligned row padding.
        let (w, h) = (128u32, 72u32);
        let bpr = ((w * 4).div_ceil(256)) * 256;
        let mut pixels = vec![0u8; (bpr * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = (y * bpr + x * 4) as usize;
                pixels[i] = 32; // B
                pixels[i + 1] = 64; // G
                pixels[i + 2] = 200; // R  (a distinctly red-ish pixel)
                pixels[i + 3] = 255;
            }
        }
        publisher.submit_frame(RawFrame {
            pixels,
            width: w,
            height: h,
            bytes_per_row: bpr,
            bgra: true,
        });

        // --- Receive it, and actually DECODE it.
        let msg = tokio::time::timeout(Duration::from_secs(5), viewer.next())
            .await
            .expect("a frame should have arrived")
            .unwrap()
            .unwrap();
        let data = msg.into_data();

        assert!(data.len() > 9, "frame must carry a payload, not just a header");
        assert_eq!(data[0], TAG_KEYFRAME, "MJPEG frames are self-contained keyframes");

        let jpeg = &data[9..];
        let img = image::load_from_memory(jpeg).expect("the payload must be a decodable JPEG");
        assert_eq!(
            (img.width(), img.height()),
            (64, 36),
            "128x72 downscaled to a 36px target height, aspect ratio preserved"
        );

        // The pixels must have survived the BGRA un-swizzle. If R and B were swapped,
        // this red-ish frame would decode blue-ish.
        let px = img.to_rgb8();
        let p = px.get_pixel(32, 18);
        assert!(
            p[0] > 150 && p[2] < 100,
            "expected a red-ish pixel (BGRA un-swizzled), got {p:?}"
        );

        assert_eq!(publisher.stats().sent.load(Ordering::Relaxed), 1);
        assert!(publisher.stats().bytes.load(Ordering::Relaxed) > 0, "bitrate must be measured");

        let _ = std::fs::remove_file(&path);
    }

    /// The resolution picker must actually change the output height. The old code
    /// prefix-matched a fixed list and silently mapped 1080p, 1440p, and 4K all to
    /// 720, so the picker was a no-op above 720p. Guard every offered option.
    #[test]
    fn resolution_string_parses_to_the_right_clamped_height() {
        assert_eq!(LiveConfig::height_from_resolution("640x360"), 360);
        assert_eq!(LiveConfig::height_from_resolution("854x480"), 480);
        assert_eq!(LiveConfig::height_from_resolution("1280x720"), 720);
        assert_eq!(LiveConfig::height_from_resolution("1920x1080"), 1080);
        // Above 1080 clamps down (MJPEG bandwidth sanity), not silently to 720.
        assert_eq!(LiveConfig::height_from_resolution("3840x2160"), 1080);
        // Below 240 clamps up; garbage falls back to 720.
        assert_eq!(LiveConfig::height_from_resolution("160x120"), 240);
        assert_eq!(LiveConfig::height_from_resolution("nonsense"), 720);
    }

    /// Only ever downscale: a 480p source asked for 720p must stay 480p rather than
    /// being upscaled into a bigger, blurrier, more expensive stream.
    #[test]
    fn never_upscales() {
        let (sw, sh) = (854u32, 480u32);
        let target = 720u32;
        let dh = target.min(sh).max(2);
        assert_eq!(dh, 480);
        let dw = ((sw as f32 * dh as f32 / sh as f32) as u32).max(2) & !1;
        assert_eq!(dw, 854, "aspect ratio preserved");
    }
}
