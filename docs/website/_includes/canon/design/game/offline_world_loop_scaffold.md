# Offline First-Person World Loop Scaffold

This document defines the engine-facing world loop contract for offline-first gameplay.

## Goals

- deterministic fixed-step world simulation
- clean separation between simulation and rendering/input adapters
- session-mode aware (offline, p2p host/join, dedicated)
- persistence hooks for snapshot + event-log hybrid storage
- CLI-playable fallback for AI and headless testing

## Fixed-step loop contract

- simulation step: fixed dt (default 1 in-game hour per command/tick in scaffold)
- input mapped to deterministic actions
- action processing updates world state and emits event entries
- periodic snapshot checkpoints

## Layer split

1. **Simulation core**
   - world state
   - action reducer
   - systems updates (lifeform/water/soil/crop/etc.)

2. **Session orchestration**
   - mode/policy/fidelity validation
   - transition rules (offline -> host -> dedicated)

3. **Presentation adapters**
   - first-person renderer (future wgpu runtime)
   - CLI adapter (current, headless test/play mode)

## Persistence contract

- snapshot backend (fast load)
- event backend (append-only audit/replay)

Scaffold currently supports JSON snapshots for rapid iteration.
Design target remains SQLite + CBOR event log.

## Why CLI mode exists

- enables AI-assisted testing/playthroughs without GUI assumptions
- supports automated regression scripts
- useful for dedicated-server/headless validation
