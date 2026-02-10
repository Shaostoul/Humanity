# Voice, Video, and Livestreaming

## Purpose

Define the architecture for real-time audio, video, and livestreaming across the Humanity Network — from 1-on-1 calls to broadcasts serving thousands of concurrent viewers — without centralizing cost or control.

## Principles

- **The server is a seed, not a bottleneck.** More viewers = more capacity.
- **P2P first.** Direct connections when possible, relayed only when necessary.
- **Users control their own bandwidth.** Configurable limits so you can game while watching a stream.
- **Many seeds, small slices.** Distribute load across as many peers as possible rather than burdening a few.
- **Progressive enhancement.** Voice works in a browser today. Video and streaming build on the same stack.

## Transport: WebRTC

All real-time media uses WebRTC:
- Native browser support (no plugins)
- Built-in encryption (DTLS-SRTP)
- NAT traversal (ICE/STUN/TURN)
- Adaptive bitrate and congestion control
- Works on web, mobile, and native clients

The relay server provides **signaling only** (exchanging SDP offers/answers and ICE candidates via the existing WebSocket connection). Media flows peer-to-peer or through an SFU — never through the chat relay.

## Tier 1: Voice Calls

### 1-on-1 Voice
- Pure WebRTC P2P between two friends
- Relay does signaling only — zero audio data touches the server
- Bandwidth: ~50kbps per direction (Opus codec)
- Latency: typically <100ms
- Requirement: mutual friends (same as DM requirement)

### Group Voice (2-8 participants)
- Full mesh P2P: each participant sends their audio to every other participant
- Works well up to ~6-8 people (upload = N-1 streams × 50kbps)
- At 8 people: ~350kbps upload per person — manageable for most connections
- Relay does signaling for all peer connections

### Group Voice (8+ participants)
- Requires an SFU (Selective Forwarding Unit)
- Each participant sends ONE audio stream to the SFU
- SFU forwards to all other participants (server-side fan-out)
- Server bandwidth: N × N × 50kbps (20 people ≈ 1Mbps)
- Open source SFUs: LiveKit, mediasoup, Janus, Pion

### Voice Channel UX
- Persistent voice channels in a server (like Discord voice channels)
- Click to join/leave
- Push-to-talk and voice activity detection (VAD) options
- Visual indicators: who's talking, who's muted
- Server admin configurable: max participants, permissions

## Tier 2: Video Calls

### 1-on-1 Video
- Pure WebRTC P2P
- Bandwidth: ~1-3Mbps per direction (VP8/VP9/AV1)
- Simulcast: sender produces multiple quality layers, receiver picks based on bandwidth

### Group Video (2-6 participants)
- Full mesh with simulcast
- Each sender produces 3 layers (low/med/high)
- Receivers subscribe to high for the speaker, low for thumbnails
- Upload: ~3-5Mbps per participant

### Group Video (6+ participants)
- SFU required
- Simulcast + selective forwarding: SFU sends high-quality for active speaker, low for others
- Server bandwidth scales linearly, not quadratically
- Dominant speaker detection: auto-switch spotlight

## Tier 3: Livestreaming

The hardest problem. One streamer, potentially thousands of viewers.

### Architecture: SFU + Peer-Assisted Mesh

```
Streamer (source)
    │
    ▼
  SFU (root seed)
    │
    ├── Viewer A (direct from SFU)
    │      ├── Viewer D (relayed by A)
    │      └── Viewer E (relayed by A)
    ├── Viewer B (direct from SFU)
    │      ├── Viewer F (relayed by B)
    │      │      └── Viewer H (relayed by F)
    │      └── Viewer G (relayed by B)
    └── Viewer C (direct from SFU)
           └── ...
```

### How it works

1. **Streamer** sends one video stream to the SFU (~3-8Mbps upload)
2. **SFU** distributes to **Tier 1 viewers** (direct connections, ~20-100 depending on server bandwidth)
3. **Each viewer re-relays to other viewers** via WebRTC data channels
4. Viewers form a **dynamic mesh** — not a rigid tree
5. Each viewer connects to **multiple peers** for redundancy (not just one parent)

### Many Seeds, Small Slices

Rather than each viewer relaying the full stream to 2-3 others (high per-peer load), use **chunked distribution**:

- Stream is segmented into small chunks (~100-500ms segments)
- Each chunk is content-addressed (BLAKE3 hash)
- Viewers download different chunks from different peers simultaneously
- Similar to BitTorrent piece selection but for live data

**Example at 4Mbps stream:**

| Upload budget | Chunks served | Peers fed (partial) |
|---------------|---------------|---------------------|
| 500kbps       | ~12.5% of stream | Contributes to many peers |
| 1Mbps         | ~25% of stream | Contributes more |
| 2Mbps         | ~50% of stream | Significant contributor |
| 4Mbps+        | Full relay | Can be a full seed |

This means someone on a 10Mbps connection gaming (using ~5Mbps) can still contribute 500kbps of relay capacity. Every little bit helps. A viewer with terrible upload (200kbps) still contributes — they just serve fewer chunks.

### Bandwidth Allocation

**The user controls everything.** No peer relay happens without consent and configuration.

#### Client-Side Bandwidth Settings

```
Bandwidth Settings
├── Total upload limit: [auto-detect | manual]  (e.g., 10 Mbps)
├── Reserved for other apps: [slider]           (e.g., 5 Mbps for gaming)
├── Available for Humanity: [calculated]        (e.g., 5 Mbps)
│
├── Relay contribution: [off | low | medium | high | custom]
│   ├── Off:    0 — receive only, never relay
│   ├── Low:    10% of available  (500kbps)
│   ├── Medium: 25% of available  (1.25Mbps)
│   ├── High:   50% of available  (2.5Mbps)
│   └── Custom: user picks exact limit
│
└── Download quality: [auto | 1080p | 720p | 480p | audio-only]
```

#### Auto-Detection
- On first use: run a quick bandwidth test (upload small chunks to relay, measure throughput)
- Store results locally
- Periodically re-test in background (brief, non-disruptive)
- Adjust relay contribution dynamically if congestion detected

#### Dynamic Throttling
- If the user's game starts lagging (detected via RTT increase to relay):
  - Automatically reduce relay contribution
  - Drop to lower quality layer for own viewing
  - Notify user: "Reduced relay to protect your connection"
- If bandwidth improves, gradually increase contribution

### Mesh Management

**Peer selection:**
- Prefer peers that are geographically close (lower latency)
- Prefer peers with higher available bandwidth
- Maintain 3-6 upstream peers per viewer for redundancy
- If a peer drops, immediately find a replacement

**Chunk scheduling:**
- Rarest-first for older chunks (ensures availability)
- Sequential-first for newest chunks (minimizes playback delay)
- Deadline-based: drop chunks that can't arrive before playback time

**Health monitoring:**
- Each peer reports its buffer health (seconds of cached content)
- Peers with low buffers get priority from high-buffer peers
- SFU acts as ultimate fallback for any peer that can't get chunks from the mesh

### Latency Profile

| Path | Latency | Use case |
|------|---------|----------|
| Direct from SFU | ~200-500ms | Tier 1 viewers, interactive streams |
| 1 peer hop | ~500ms-1s | Most viewers |
| 2-3 peer hops | ~1-2s | Edge of mesh |
| 4+ hops | ~2-4s | Maximum reach |

For non-interactive streams (watching a concert, presentation), 2-4 seconds is perfectly acceptable. For interactive streams (Q&A, gaming with chat), keep the mesh shallow.

### Adaptive Bitrate

Streamer produces multiple quality layers via simulcast or SVC (Scalable Video Coding):

| Layer | Resolution | Bitrate | Use |
|-------|-----------|---------|-----|
| High | 1080p | 4-6Mbps | Full quality viewers |
| Medium | 720p | 1.5-2.5Mbps | Default |
| Low | 480p | 500-800kbps | Bandwidth-constrained |
| Audio-only | — | 64kbps | Extreme constraints |

Peers relay whichever layers they can afford. A low-bandwidth peer might only relay the audio layer — still helpful.

### Server Cost at Scale

Traditional CDN: cost grows linearly with viewers.
Peer-assisted: cost plateaus.

| Viewers | Traditional server BW | Peer-assisted server BW |
|---------|----------------------|------------------------|
| 10 | 40Mbps | 40Mbps (no difference) |
| 100 | 400Mbps | ~60Mbps |
| 1,000 | 4Gbps | ~80Mbps |
| 10,000 | 40Gbps | ~100Mbps |
| 100,000 | 400Gbps (💀) | ~150Mbps |

The server only needs to feed enough Tier 1 viewers to seed the mesh. Everything else is peer-relayed.

## Recording and Playback

- Streams can be recorded server-side (streamer opt-in)
- Recordings stored as content-addressed blocks (same as file sharing)
- Playback uses the same P2P chunk delivery as live streaming
- Popular recordings get faster as more people watch (more seeds)

## Privacy and Safety

- Voice/video streams are encrypted end-to-end (WebRTC DTLS-SRTP)
- SFU can see stream metadata but ideally uses insertable streams (E2EE through the SFU)
- Relay contribution is always opt-in
- IP addresses visible to direct peers (standard WebRTC limitation)
  - TURN relay available for users who want IP privacy (at cost of latency)
- Stream recordings require streamer consent
- Viewers cannot record without client modification (no DRM — intentional)

## Implementation Phases

1. **Phase 1 — Voice P2P:** WebRTC signaling through existing WebSocket relay. 1-on-1 voice between friends. Browser + native.
2. **Phase 2 — Group voice:** Full mesh for small groups. Voice channels in server UI.
3. **Phase 3 — Video calls:** Add video tracks to existing WebRTC connections. Simulcast.
4. **Phase 4 — SFU integration:** Self-hosted LiveKit or similar for larger groups.
5. **Phase 5 — Livestreaming MVP:** SFU + basic peer relay tree. Bandwidth settings UI.
6. **Phase 6 — Chunked mesh delivery:** BitTorrent-style chunk distribution for streams. Many-seed architecture.
7. **Phase 7 — Adaptive optimization:** Dynamic throttling, geo-aware peer selection, congestion avoidance.

## Open Questions

- Should voice channels be server-scoped or cross-server (like DMs)?
- Screen sharing: separate feature or extension of video calls?
- Should the SFU be built into the relay binary or a separate service?
- TURN server hosting: self-hosted vs. commercial (Twilio, Cloudflare) for NAT traversal?
- Maximum mesh depth before mandating SFU fallback?
- Should stream recordings count toward user storage quotas?
