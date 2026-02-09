# Realtime Relay Protocol

## Purpose
Define the WebSocket relay protocol frames, subscriptions, and delivery semantics.

Relays provide realtime delivery and connectivity fallback.
They deliver immutable signed objects and feed cursors; they do not define truth.

## Connection requirements
- TLS required.
- Client must authenticate to access non-public spaces.
- Web clients must be supported.

## Frame format
All frames are JSON for transport simplicity.
Frame payloads must not be used for hashing/signing; they carry object identifiers and optional inline objects.

Each frame:
- type: string
- request_id: optional string
- payload: object

## Client to relay frames

### authenticate
Payload:
- access_token: string
- device_public_key: bytes (optional)

Relay responds with:
- authenticated or error

### subscribe_space_feed
Payload:
- space_id: string
- cursor: optional string
- include_objects: optional boolean

### subscribe_channel_feed
Payload:
- channel_id: string
- cursor: optional string
- include_objects: optional boolean

### publish_object
Payload:
- objects: list of signed objects (binary encoded as base64 or hex)

Relay validates:
- session
- rate limits
Relay may forward to server acceptance pipeline.

### ping
Payload:
- timestamp: integer (informational)

## Relay to client frames

### feed_event
Payload:
- scope_type: "space" or "channel"
- scope_id: string
- object_ids: list of string
- objects: optional list of inline objects
- next_cursor: string

### publish_result
Payload:
- results: list
  - object_id: string
  - status: "accepted" or "rejected"
  - reason: optional string

### error
Payload:
- code: string
- message: optional string

## Delivery semantics
- At-least-once delivery.
- Client must deduplicate by object_id.
- Ordering is best-effort; authoritative order comes from server feeds.
- Relay may coalesce events to reduce bandwidth.

## Abuse controls
- Per-identity and per-connection rate limits.
- Connection quotas.
- Backpressure:
  - relay may drop optional frames (presence/typing) first
  - relay must not drop durable feed events silently; if overloaded it must disconnect with explicit error

## Optional ephemeral signals
Presence and typing may be supported as non-durable frames.
They must not be required for basic operation.
