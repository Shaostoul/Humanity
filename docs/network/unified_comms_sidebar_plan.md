# Unified Comms Sidebar Plan (Left + Right Collapsible Structure)

## Goal
Unify stream/watch/voip presence controls in one coherent comms window while preserving active VOIP across tabs.

## Right Sidebar target structure

- Friends
  - Streaming (with inline preview tile)
  - VOIP
  - Online
  - Offline
- Groups (per group)
  - Streaming
  - VOIP
  - Online
  - Offline
- Servers (per server)
  - Streaming
  - VOIP
  - Online
  - Offline

Each section is collapsible and keeps per-section user toggles:
- watch/hide stream
- pause stream to save bandwidth while staying in voice
- quick mute/deafen controls

## Left Sidebar target structure

Replace current 3-tab switcher with collapsible sections mirroring context:
- Servers
- Groups
- DMs

Each section shows active context and voice/stream indicators.

## Streaming behavior

- Stream watch defaults to off unless user enables auto-watch.
- Stream and voice controls are decoupled:
  - user can remain in VOIP while pausing stream video.
- Pinned in-app preview is context-aware and tied to selected stream source.

## Implementation phases

1. Data model: normalize stream/voip presence by context (friend/group/server)
2. Right sidebar: collapsible grouped renderer with inline preview rows
3. Left sidebar: collapsible context navigation
4. Controls: watch/pause/quality/source toggles in one pane
5. Persist user preferences by context and device

## Accessibility and reliability

- keyboard navigable collapsible sections
- deterministic state restoration on tab/context change
- no media teardown when switching visual tabs (unless user leaves room)
