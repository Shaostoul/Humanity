# RFC: Studio Mode (Multi-Source Streaming + BRB + Vtuber Support)

Status: Draft

## Summary
Studio Mode upgrades Humanity streaming from single-source capture to a composited, creator-first pipeline supporting multiple video/audio sources, BRB overlays, and vtuber workflows via virtual cameras.

## Goals
- Multi-source video + audio mixing in-app.
- One-tap BRB workflow with AFK timer overlay.
- Native virtual camera compatibility for vtubers.
- Stable desktop-first behavior with mobile-safe controls.

## Scope (Phase 1)
1. Source stack:
   - screen/window capture
   - webcam/virtual webcam source
   - optional second camera source
2. Audio stack:
   - default mic + optional extra mic
   - desktop audio
   - per-source mute/gain
3. BRB mode:
   - Go BRB / Back toggle
   - on-stream timer overlay
   - optional auto-mute
4. Presets:
   - save/load scene presets (e.g., Gameplay, BRB, Chatting)

## Vtuber Compatibility
- Treat virtual camera as first-class video source.
- Add source favorites to quickly select avatar cam.
- Ensure label normalization + persistent preferred source.
- Future: transparent/alpha-friendly compositing options.

## Architecture
- Compositor graph:
  - base layer: screen/window
  - overlays: webcam/avatar/chat/labels
- Audio mixer graph:
  - destination track from mixed inputs
- Scene state:
  - serialized JSON for layout + source states

## UX
- Stream controls:
  - Sources
  - Audio Mixer
  - Scenes
  - BRB
- Safety:
  - live-state guard for tab switching
  - explicit source missing warnings

## Milestones
- M1: Multi-source selection + audio mixer basics
- M2: BRB overlay + timer + auto-mute
- M3: Scene presets and quick switching
- M4: Vtuber profile + favorites + diagnostics

## Non-goals (initial)
- Full OBS parity
- Browser-level removal of system sharing prompts/borders
- Cloud transcoding orchestration
