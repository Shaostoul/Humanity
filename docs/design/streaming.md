# Streaming: capture, encode, transport

> **Status:** design + build plan (2026-07-14). Written from a 4-agent research pass
> against the real code. Studio's "Go Live" is currently a rehearsal: it flips three
> booleans and nothing leaves the machine. This is the plan to make it real.

## The one-paragraph summary

The **transport is nearly free** and the **encoder is the blocker**. str0m 0.20 (already
a dependency) ships RTP packetizers for H264/VP8/VP9/AV1 and enables them by default, so
the WebRTC send path for video is roughly an 80-line diff against the voice path that
already ships. What does NOT exist is a way to turn frames into H.264 without a C
toolchain, which this project has deliberately refused twice (str0m `rust-crypto` instead
of `aws-lc-sys`; `unsafe-libopus` instead of `libopus-sys`). So the plan is: ship a real,
self-hosted live stream FIRST using an encoder we already have (MJPEG via the `image`
crate), over a transport that is trivially upgradeable, and treat the hardware H.264
encoder as the next rung rather than a prerequisite.

## What already exists (verified in-tree)

| Thing | Where | Reusable? |
|---|---|---|
| **Full real-time audio pipeline** | `src/net/voice.rs` | YES. cpal capture, resample, RNNoise, gate, **Opus encode** (`unsafe-libopus`), ring buffer, engine pump, RTP write via str0m. This is a complete, working media path. |
| **WebRTC send/recv, ICE/STUN/TURN** | `src/net/webrtc.rs` | YES, codec-agnostic. Adding video = `add_media(MediaKind::Video, SendOnly)` + a `cmd_send_video` mirroring `cmd_send_voice` with `Frequency::NINETY_KHZ`. |
| **str0m video packetizers** | str0m 0.20 `packet/{h264,vp8,vp9,av1}.rs` | YES. Already enabled by `Rtc::builder()`. The video transport is paid for. |
| **Browser already streams video** | `web/chat/chat-voice-webrtc.js`, `chat-voice-streaming.js` | YES. The WEB client already does camera + screen-share over WebRTC in voice rooms, with bitrate control. Nobody connected it to Studio. |
| **GPU frame capture** | `src/renderer/mod.rs` `capture_current_frame` / `read_texture_to_png` | PARTIALLY. It allocates a fresh ~8 MB readback buffer per call and then does `device.poll(Maintain::Wait)`, a full GPU stall. Fine for a screenshot; it will tank the framerate if called per frame. Needs async double/triple-buffered readback. |
| **Studio scenes/sources** | `src/gui/pages/studio.rs`, `data/studio/*.json` | The UI + data model exist. The canvases are `rect_filled` placeholders; no device is ever opened. |

## What does NOT exist (the honest gaps)

1. **No video encoder.** This is THE blocker. See the encoder table below.
2. **The relay WebSocket is text-only** (`src/relay/relay.rs` `if let Message::Text(text) = msg`), with a 128 KB cap and a Fibonacci rate limiter. Binary frames are silently dropped. Video must NOT be pushed through it, and must NOT go through `broadcast_tx` (a JSON enum re-serialized per socket). It needs a **new, separate binary route**.
3. **The relay has no str0m at all.** `Cargo.toml`'s `relay` feature omits it (str0m is native-only). "We already use str0m" is true of the app and FALSE of the server. Any SFU/WHIP plan starts by adding it plus public UDP ingress (nginx fronts TCP/443 only).
4. **Studio settings are not persisted** (bitrate/resolution/fps/server URL are lost on restart).

## The encoder problem, stated plainly

| Option | License | Real-time? | Build dependency | Verdict |
|---|---|---|---|---|
| openh264 | BSD-2 | yes | **C++ compiler** | Violates the no-C rule (same class as aws-lc-sys) |
| libvpx (VP8/VP9) | BSD | yes | **C build** | Violates |
| x264 | **GPL** | yes | C build | License problem for a CC0/permissive project. Double no. |
| ffmpeg | - | yes | the whole C world | No |
| **rav1e** (AV1) | BSD-2 | **no** (without asm) | asm feature needs **NASM** | Only serious pure-Rust encoder, but not real-time without the exact dependency we refused. Research spike, not a plan. |
| **Media Foundation H.264 MFT** via the **`windows` crate** | MIT/Apache | **YES** (hardware; NVENC on the RTX 4070) | **none** - `windows` is pure generated Rust bindings, and is ALREADY in the graph via cpal | **The only constraint-compatible real-time path.** Windows-only. A lot of COM. |
| **MJPEG** via `image` 0.25 (already a dep) | - | yes (cheap) | none | Bandwidth-hostile (10-20x H.264) but **zero new dependencies**. The de-risking step. |

**Conclusion:** MJPEG now, Media Foundation H.264 next. Both ride the SAME transport, so the
upgrade is a payload swap, not a rewrite.

## Build plan

### Rung 1 - MAKE "GO LIVE" ACTUALLY BROADCAST (self-hosted, no encoder dependency) - SHIPPED v0.853-v0.854

Done. `src/relay/live.rs` (fanout), `src/net/live.rs` (publisher), `src/renderer/stream_capture.rs`
(non-blocking readback), `web/pages/watch.html` (viewer). Routes are `/ws/live/{pub,sub}` and
`/api/live`. Proven end to end by a test that decodes the received JPEG, plus a real-GPU readback
test. What follows is the original plan, kept for the record.


A dedicated binary WebSocket live route on the relay, MJPEG frames from the app, a browser
viewer. This is the fastest path to a real stream the operator can point people at, and the
transport is identical to the one H.264 will use.

- **Relay** (new, isolated - does NOT touch `relay.rs`):
  - `GET /live/pub/{stream}` - publisher WS. Gated with the existing
    `verify_dilithium_signature` (`purpose\ntimestamp` preimage) so only the operator can publish.
  - `GET /live/sub/{stream}` - viewer WS. Unauthenticated.
  - Per-stream `broadcast::Sender<Arc<[u8]>>` - **bytes, not `RelayMessage`**.
  - **Cache the last keyframe + codec config** and send it to each joining viewer immediately.
    Without this a new viewer stares at a black canvas for up to a full GOP. This is the single
    detail that makes or breaks it.
  - `GET /api/live/{stream}` - is it live, viewer count.
- **Wire format** (no library): `[1B tag][8B PTS micros][payload]`, tag = 0 codec-config,
  1 keyframe, 2 delta, 3 audio. Same envelope for MJPEG and later H.264.
- **App**: non-blocking frame capture (buffer pool, drop `Maintain::Wait`, drop PNG), JPEG
  encode via `image::codecs::jpeg::JpegEncoder`, send binary WS frames. 720p at 15 fps first.
- **Web viewer**: `/live/{stream}` page. v1 draws `createImageBitmap(blob)` to a canvas.
  v2 swaps to WebCodecs `VideoDecoder` for H.264 - about 10 lines of JS change.
  (WebCodecs footgun: **call `frame.close()`** in the output callback or the tab OOMs.)
- nginx must proxy the new WS route with buffering disabled.

### Rung 2 - REAL CODEC: Media Foundation H.264 + real capture

- `windows` crate: **Windows.Graphics.Capture** (screen) + **Media Foundation H.264 MFT**
  (hardware encode). Same dependency solves capture AND encode.
- `nokhwa` (`default-features = false`, `features = ["input-msmf"]`) for camera. The default
  `decoding` feature pulls mozjpeg-sys (C + NASM) - it MUST stay off.
- **Target-gate all of it** under `[target.'cfg(windows)'.dependencies]`: CI also builds
  linux-x64 and macos, and an ungated Windows-only crate turns those red.
- RGBA -> NV12 conversion belongs in a wgpu compute pass, not on the CPU.
- Honest cost: Windows-only on day one; Linux/macOS Studio stays a rehearsal UI until someone
  writes VAAPI/VideoToolbox backends.

### Rung 3 - SCALE: HLS, then maybe an SFU

- **VPS egress is the real ceiling, not the protocol.** 4 Mbps x 25 viewers = 100 Mbps
  sustained, which saturates a typical VPS. Every unicast option has this problem identically;
  WebRTC does not fix it. **HLS + nginx caching does.**
- HLS: relay muxes fMP4 segments to disk and serves them with `ServeDir` (the `/uploads`
  pattern already exists); viewers use hls.js, native on Safari/iOS. Needs a small hand-rolled
  fMP4 muxer (a few hundred lines, pure Rust). 6-10 s latency, so it complements rather than
  replaces Rung 1. Gives VOD for free by keeping the segments.
- SFU (str0m in the relay + UDP ingress) is the correct sub-second-at-scale answer but is real
  work. Do not block go-live on it.

### NOT doing (and why)

- **RTMP to Twitch/YouTube.** Pure-Rust RTMP clients exist (`rml_rtmp`, MIT, sans-IO - a good
  fit). But RTMP requires **AAC**, and there is no pure-Rust AAC encoder; `fdk-aac` is a C
  `*-sys`. The only constraint-compatible option is a Windows-only Media Foundation AAC
  encoder. It is a separate project, and it does not serve the self-hosted architecture the
  Studio's own defaults already declare (`Platform = HumanityOS Server`).

## Security finding (unrelated to the above, found during the research)

**The TURN long-term credential is committed in plaintext** at `src/net/webrtc.rs` and served
publicly in `web/chat/chat-voice-rooms.js`. Anyone who reads the repo or the JS can use the
operator's TURN relay as free bandwidth. Fix: rotate it, and issue short-lived TURN credentials
from the relay (the standard REST ephemeral-credential pattern, HMAC of a timestamp) rather
than shipping a static shared secret to every client.
