# Realtime Transport

## Purpose
Define realtime delivery for chat, notifications, and live updates across native and web clients.

## Transport ladder
1. Direct peer-to-peer when available.
2. Relay-assisted peer-to-peer when direct connectivity fails.
3. Server relay as mandatory fallback.

## NAT and browser constraints
- Many clients cannot accept inbound connections due to common network configurations.
- Web browsers have restricted networking capabilities and must assume relay-first operation.

## Protocol requirements
- Messages are immutable signed objects.
- Transport delivers objects; storage and verification are separate.
- Duplicate delivery must be tolerated.
- Ordering is best-effort in transport; authoritative ordering is defined by server for shard-visible streams.

## Server relay requirements
- WebSocket endpoint for web and fallback.
- Abuse protection:
  - rate limits
  - per-identity throttling
  - per-space policies
  - connection quotas
- Relay must refuse forwarding content that violates signed moderation logs where possible.

## Presence and typing indicators (optional)
- Treated as ephemeral signals.
- Not stored as durable history by default.
