# Session Modes & Authority Model

This document defines how Humanity supports:

- offline singleplayer
- LAN co-op
- direct-internet P2P co-op
- optional dedicated server (open/closed progression policies)

## 1) Session modes

- **Offline**
  - Authority: local machine
  - Persistence: local save + local event log
  - Network: none

- **Host P2P**
  - Authority: host player machine
  - Persistence: host save/event log
  - Network: LAN or direct internet invite keys

- **Join P2P**
  - Authority: remote host
  - Persistence: host-owned world state
  - Network: LAN/direct internet/tailnet

- **Dedicated Server**
  - Authority: server process
  - Persistence: server save/event log
  - Network: public internet and/or private network

## 2) Progression policies

Inspired by open-net vs closed-net distinctions.

- **Open Profile Policy**
  - local progression can be imported
  - higher flexibility
  - higher cheating risk

- **Closed Profile Policy**
  - progression is server-authoritative only
  - local client progression cannot overwrite server stats
  - preferred for competitive or persistent shared worlds

## 3) Identity portability

Character appearance/cosmetics are portable across all modes.
Progression and economy state are policy-dependent:

- open profile: portable with validation
- closed profile: server source of truth only

## 4) Dynamic mode transitions

Required transitions:

1. Offline -> Host P2P
2. Host P2P -> Dedicated Server handoff
3. Dedicated Server snapshot export -> Offline sandbox (optional, policy controlled)

Transition safety requirements:

- schema version compatibility checks
- deterministic snapshot hash checks
- policy gate checks (open vs closed)
- migration journal entry in event log

## 5) Main menu UX

Single page with clear options:

- Continue Offline
- Host Session (LAN / Internet Invite / Tailnet)
- Join Session (Code/Invite)
- Dedicated Server
  - Open profile
  - Closed profile

Advanced toggle panel:
- realism difficulty (Baby/Creative .. Realistic)
- anti-cheat policy indicator
- network exposure indicator

## 6) Security baseline

- invite keys must be high entropy and expiring
- host/dedicated modes should support allowlists
- closed profile mode requires server-side progression validation
