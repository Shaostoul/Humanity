# Native Voice (implementation reference)

> Shipped v0.485 to v0.495 (2026-06-18 to 06-21). This is the concrete
> implementation of native voice chat. For the broader aspirational media
> architecture (video, streaming, SFU tiers) see
> [voice_video_streaming.md](voice_video_streaming.md). For the WebRTC data path
> it builds on, see [p2p-groups.md](../design/p2p-groups.md).

## What it is

Native (the desktop Rust app) can now join a channel's voice room and talk, both
to other native users and to **web** users, over the same relay. It is 100%
pure-Rust (no C toolchain), matching the rest of the project's crypto/codec
posture: `cpal` (audio IO, WASAPI on Windows), `unsafe-libopus` (Opus, libopus
transpiled to Rust), `nnnoiseless` (RNNoise, pure-Rust port), `rtrb` (wait-free
ring buffer), and `str0m` (sans-IO WebRTC, rust-crypto backend).

Voice is **per text channel**: every channel can have voice enabled (the
`voice_enabled` flag, toggled in the channel admin menu). Clicking the channel's
mic joins *that channel's* voice room; the roster shows who is in it under the
channel. There is no separate "voice channels" concept (a legacy `voice_channels`
table still exists but is no longer the source of truth).

## The pipeline

```
  mic ─cpal─▶ resample→48k ─▶ gain ─▶ high-pass ─▶ noise filter ─▶ transmit gate
                                                                        │ (Opus encode)
                                                                        ▼
                                                    str0m WebRTC ──▶ each connected peer
                                                                        │ (peer's Opus)
  speaker ◀─cpal─ resample→devrate ◀─ mix ◀─ per-peer Opus decode ◀────┘
```

Everything runs on one worker thread (`run_voice_session` in
[src/net/voice.rs](../../src/net/voice.rs)), paced at ~5 ms. The capture half and
the playback half share the same DSP params (read live from atomics, so settings
changes apply without restarting).

## Phases (how it was built)

| Phase | Version | What landed |
|------|---------|-------------|
| A | v0.485-487 | Mic loopback test (hear yourself). cpal + unsafe-libopus + rtrb. Mic-test UX: toggle, animated RGB border, live level meter, input/output device pickers. Accepts ANY device sample format (i16/u16/f32) + ANY rate (streaming linear resampler). |
| Input | v0.488 | Gain 0-200%, filter modes (Off / Light / Noise suppression), transmit modes (Open mic / PTT / Voice-activated / Push-to-mute), all persisted. |
| B | v0.489 | str0m bidirectional Opus media (`add_media` audio m-line + `Writer::write` + `Event::MediaData`), opt-in on the existing per-peer Rtc. |
| Defaults | v0.490 | Default filter = Noise suppression (RNNoise); default transmit = Push-to-talk on CapsLock (raw winit key handling, since egui has no CapsLock and PTT must work in-game). |
| C1 | v0.491 | Native voice-room JOIN registers with the relay. |
| C2 | v0.492 | Native WebRTC signaling over `voice_room_signal`, interoperable with web. |
| Per-channel | v0.493 | Voice room = the text channel (keyed by channel id). Fixed "clicking the mic does nothing". |
| D | v0.494-495 | Live audio: send mic to peers, decode + mix + play inbound. native↔web audible both ways. |

## DSP (the input chain)

All in [src/net/voice.rs](../../src/net/voice.rs), exposed in Settings → Audio →
Voice ([src/gui/pages/settings.rs](../../src/gui/pages/settings.rs)
`draw_audio_content`). Chain order: **gain → high-pass → noise filter → transmit
gate → Opus**.

- **Gain** `0-200%` (1.0 = unchanged), clip-protected.
- **Filter modes** (`VoiceFilterMode` in [src/config.rs](../../src/config.rs)):
  - *Off* - raw mic.
  - *Light* - ~85 Hz biquad high-pass (kills rumble/hum) + a soft noise gate.
  - *Noise suppression* (**default**) - RNNoise via `nnnoiseless`. Removes
    keyboard clicks, coughs, fans, and steady background noise *even while you
    speak* (a gate only helps between words). RNNoise wants i16-range f32 in
    480-sample frames; the `Denoiser` wrapper scales x32768 and chunks the 960
    Opus frame into two 480 frames.
- **Transmit modes** (`VoiceTransmitMode`): Open mic, Push-to-talk (**default**,
  CapsLock), Voice-activated (RMS threshold + hangover), Push-to-mute. The push
  key is read from raw winit input (`voice_ptt_held`), so it works in-game.
- 6 DSP unit tests (resampler, gain clamp, high-pass DC removal, RNNoise runs,
  transmit-mode gating).

## Transport (WebRTC via str0m)

[src/net/webrtc.rs](../../src/net/webrtc.rs). The audio m-line is added to the
**existing** per-peer `Rtc` (one ICE/DTLS/SRTP transport carries data + audio),
opt-in via `offer_to_voice(peer, room_id)` so the P2P-groups data mesh is
untouched (a regression test asserts the data-only offer has no audio m-line).

- **Send:** `send_voice(peer, opus)` → `rtc.writer(mid).write(pt, now, MediaTime,
  opus)`. The Opus payload type is discovered from `writer.payload_params()`
  (str0m may reassign it; never hardcode 111). A per-peer 48 kHz RTP clock
  advances 960 per 20 ms frame.
- **Receive:** `Event::MediaData` → `WebrtcEvent::VoiceFrame{peer, opus}`.
- **Connect:** `Event::IceConnectionStateChange(Connected)` →
  `WebrtcEvent::VoiceConnected{peer}`.

### Signaling protocol (matches the web client)

Voice signaling rides `voice_room_signal` (distinct from the P2P-groups
`webrtc_signal`), with `data` as a JSON **object** (the browser
RTCSessionDescription / candidate shape), not a string. str0m serializes
`SdpOffer`/`SdpAnswer` to `{type, sdp}`, which the browser reads directly; no SDP
munging; on-the-wire codec is browser-default Opus 48 kHz, which str0m's default
Opus matches.

- **Join:** client sends `{type:"voice_room", action:"join", room_id:"<channel
  id>"}`. The relay validates the channel's `voice_enabled` flag, adds the client
  to the in-memory `voice_rooms[channel_id]`, sends each existing member a
  `new_participant` signal, and broadcasts `voice_channel_list` (the roster).
- **Glare rule:** *newcomer offers, incumbents wait.* The joiner offers to every
  member present in its first post-join roster; later joiners offer to it. No key
  tiebreak. Native tracks this via `voice_active_room` + `voice_incumbents_captured`
  in [src/lib.rs](../../src/lib.rs).
- **Offer/answer/ice:** `{type:"voice_room_signal", from, to, room_id,
  signal_type:"offer"|"answer"|"ice", data:<object>}`. The relay forwards only
  between two confirmed co-members.
- Relay handlers: `handle_voice_room` + `handle_voice_room_signal` in
  [src/relay/handlers/msg_handlers.rs](../../src/relay/handlers/msg_handlers.rs);
  roster in [src/relay/handlers/broadcast.rs](../../src/relay/handlers/broadcast.rs)
  (`build_voice_channel_list_msg`, built from voice-enabled text channels, id is a
  String).

## Key files

| File | Role |
|------|------|
| `src/net/voice.rs` | Opus encode/decode, DSP (gain/high-pass/gate/VAD/RNNoise), the loopback test, and `run_voice_session` (capture+encode+send queue, receive+decode+mix+play). |
| `src/net/webrtc.rs` | str0m driver; audio m-line, `send_voice`, voice signaling (`cmd_voice_signal`/`on_voice_offer`/`emit_voice_signal`), `VoiceConnected`/`VoiceFrame` events. |
| `src/gui/pages/settings.rs` | `draw_audio_content`: device pickers, mic test, gain, filter + transmit selectors, push-key binder. |
| `src/config.rs` | `VoiceFilterMode`, `VoiceTransmitMode`, persisted voice prefs. |
| `src/lib.rs` | Per-frame param push, session lifecycle, roster-driven offers, send pump, VoiceFrame → playback. |
| `src/relay/handlers/{msg_handlers,broadcast}.rs` | voice_room join/leave + voice_room_signal relay + roster. |

## Known limitations / TODO

- **No per-peer controls UI on native** yet (volume / mute / squelch). The web has
  this (`web/chat/chat-voice-modal.js`); native should mirror it.
- **The web has no transmit-mode UI** (open mic / PTT / VAD). Web-parity debt.
- **No in-process test** for the WebRTC voice path - it is verified only by a live
  native↔web call. The agreed next infra build is a two-`str0m`-in-one-process
  harness (str0m is sans-IO, so two instances can be wired together with a fake
  clock and no sockets) to make voice/net changes CI-verifiable.
- **Playback is naive:** a per-peer sample queue + sum-mix, no adaptive jitter
  buffer / drift compensation. A residual faint click under heavy clock drift is
  possible; revisit with proper drift handling if it appears.
- **Deploys break active voice:** every release restarts the relay (clears the
  in-memory `voice_rooms`, drops all client WebSockets) and the web force-reloads
  on the `server_version` change. Batch deploys; a graceful relay restart (drain /
  persist voice rooms) is a future improvement.
