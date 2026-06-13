# Studio + Streaming: design

> **Status:** design (2026-05-20). Captures the operator's detailed studio/streaming/viewer vision + the cross-platform sequencing. Large multi-phase arc, this doc is the source of truth so the spec isn't lost between increments.

## The vision (operator, 2026-05-20)

1. **Web mirrors native.** The native chat page is the canonical design ("great, loving it"); the web chat is the old/broken UI and must be rebuilt as a *child* of the native parent, same GUI/UX/pages/style/theme. egui is less capable than HTML/CSS, so native is designed within its constraints and web mirrors (web may not exceed/diverge). This is already the CLAUDE.md "Rust-first canonical UI" rule; the gap is execution (web chat is stale).
2. **Studio widget moves to the RIGHT rail**, not the left. For a streamer (the operator), it sits at the **top, above Friends**.
3. The studio widget holds **commonly-used buttons** + a button to open a **large Studio modal** (full streaming control surface: analytics, metrics, buttons, configs, dropdowns, sliders, variables, settings).
4. **Chat docked under the studio widget**: the studio area has a chat at its bottom (e.g. the operator's relay #general), with the **text-entry at the TOP** (closest to the studio controls) and messages **newest-at-top → oldest-at-bottom** (inverted from the normal chat page).
5. Below the studio area: the normal chat-page **Friends list**. Under each **streaming friend**: a **simplified viewer widget**, a paused video frame (last loaded frame from their stream).
6. A **dedicated viewer modal** for focusing on one/multiple streams: video on top (where the studio widget is), chat on the bottom.
7. **Persistent streaming.** Streaming must continue even when the operator leaves the Studio page, navigating to ANY HumanityOS page keeps the stream live. (Global session, not page-bound.)
8. **Privacy guard.** While streaming, if the user navigates to a sensitive page and clicks a dangerous action (e.g. "view my seed phrase"), auto-hide/obscure the stream so secrets never go out live.

## Current state (the inverted maturity)

| | Web | Native |
|---|---|---|
| Chat UI | OLD / broken (pre-redesign) | Polished (the parent design) |
| Streaming transport | **Real WebRTC** (`chat-voice-webrtc.js` 30K, `chat-voice-streaming.js` 15K, `chat-voice.js` 105K, rooms 44K) | **None**, `stream_*` events are no-op stubs (v0.283.0) |
| Studio surface | A "Studio" section, bottom-LEFT of chat | A full Studio **page** (`studio.rs`, scenes/sources/config), but no transport behind it, and no widget in the chat page |
| Relay support | `stream_*` RelayMessages exist; relay relays signaling | same (relay is platform-agnostic) |

**The crux:** the operator primarily uses native, but native can't stream. Web can stream but looks old. So:
- The studio/streaming *widget* can be built+functional on **web first** (transport exists), then mirrored to native once native's transport is built.
- "Always stream on every page" + viewer widgets are **gated on a page-persistent stream session**, the hard dependency on both platforms (and on native, on the WebRTC stack not existing yet, the same weeks-long effort flagged for native voice in PRIORITIES TIER 2).

## Target design: details

### Studio widget (right rail, top, streamer view)
- Replaces the bottom-left placement. Right rail order for a streamer: **[Studio widget] → [Friends list] → [server members]**.
- Compact: live/offline indicator, viewer count, a few common buttons (start/stop, mute, scene switch), and an **"Open Studio" button** → the modal.
- Non-streamers don't see the studio widget (or see a minimal "Go Live" entry point).

### Studio modal (full control surface)
- Large modal. Content ≈ the existing native `studio.rs` page (scenes, sources, streaming config) PLUS analytics/metrics (viewers, bitrate, uptime, dropped frames), and all the configs/dropdowns/sliders/variables.
- The native Studio *page* content is the starting material, refactor it to render inside a modal AND keep a full-page route.

### Docked studio chat (inverted)
- Under the studio widget: a chat bound to a chosen channel (default the relay #general). **Input at top, newest-at-top.** This is a distinct render mode from the main chat page (which is input-at-bottom, newest-at-bottom).

### Viewer widget (under each streaming friend)
- A small "paused video" tile showing the last decoded frame (a poster/thumbnail) from that friend's stream. Click → the viewer modal.
- Needs the transport to expose a "latest frame" per remote stream.

### Viewer modal (multi-stream focus)
- Video(s) on top, chat on bottom. Focus one or tile multiple. Reuses the docked-chat component.

### Persistent stream session (the foundation)
- The capture + outbound stream lives in a **global session object** owned above the page router, not in the Studio page's lifecycle. Page navigation re-parents the *preview* UI but never tears down the session.
- Web: hoist the existing WebRTC session out of the Studio-section scope into a page-independent singleton (the `shared/` layer). Native: requires the transport to exist first.

### Privacy guard
- A global "sensitive context" flag. Pages/actions that expose secrets (Identity/Recovery seed reveal, vault unlock, key export) raise it.
- While streaming + flag raised: the outbound video is replaced with a "Privacy screen, sensitive content hidden" frame (and ideally the capture is paused, not just overlaid, so nothing leaks even via a race).
- Highest-value, smallest, most-independent piece, it's a guard layer over capture, doesn't need the full widget redesign.

## Phased plan

**Track W, web-mirrors-native (independent, ongoing).** Rebuild the web chat UI to match the native parent: theme tokens (already shared via `theme.css` ← `theme.ron`), layout, panels, components. Page-by-page. Big; runs in parallel with everything else. Start with the chat page (the one the operator contrasted).

**Track S, studio/streaming (dependency-ordered):**
- **S0, persistent stream session.** Web: hoist WebRTC to a page-independent singleton so the stream survives navigation. (Native S0 = build the WebRTC transport, the weeks-long item; do native after web proves the design.) **This is the gate for #7 + viewers.**
- **S1, studio widget + modal (web).** Right-rail placement, common buttons, the full modal (port the studio.rs content), docked inverted chat. Functional because web has transport.
- **S2, viewer widgets + viewer modal (web).** Paused-frame tiles under streaming friends; multi-stream focus modal.
- **S3, privacy guard.** Global sensitive-context flag + capture-pause/overlay. Can land early (parallel to S1) since it's independent.
- **S4, native mirror.** Once web design is settled AND native transport exists, mirror the widget/modal/viewer to native egui.

**Recommended first build:** **S3 (privacy guard)**, small, safety-critical, mostly independent, protects the operator the moment they stream, OR **S1 web studio widget** (most directly matches the detailed layout ask; visible; functional on web). S0 (persistent session) is the deepest dependency and the right thing to do before S2/viewers.

## Honest scope note

This is not one increment. The full vision is a multi-week arc per platform, and the native half is gated on the WebRTC transport that doesn't exist yet (same lift as native voice). The web half can move now because the transport exists, but the web chat *also* needs the parity rebuild (Track W) to stop looking old. Sequencing both at once is a lot; pick the entry point deliberately.

## Open questions for the operator

1. **Native streaming transport**, green-light the WebRTC build for native now (weeks), or keep native streaming as "view-only / not yet" and make the web client the streaming surface for now?
2. **Web parity rebuild scope**, full chat-page rebuild to match native, or incremental component-by-component? (Incremental is safer; it keeps the working web chat usable throughout.)
3. **Entry point**, privacy guard first (safety), web studio widget first (matches the detailed ask), or persistent-session first (the foundation)?
