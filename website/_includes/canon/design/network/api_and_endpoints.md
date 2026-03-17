# API and Endpoints

## Purpose
Define minimum API surfaces required to support web, desktop, and game clients with offline-first sync.

## Principles
- Server verifies signatures; server does not decrypt identity keys.
- Endpoints must be idempotent where possible.
- Clients must be able to resume sync from cursors.
- Rate limits are mandatory on all write endpoints.

## Authentication and sessions
### POST /auth/login
- establishes a session
- returns short-lived access token bound to device and identity

### POST /auth/refresh
- refreshes short-lived token

### POST /auth/logout
- invalidates current session

## Device enrollment and revocation

### POST /devices/enroll
- enroll a new device public key
- requires authenticated session and user confirmation

### POST /devices/list
- returns enrolled devices

### POST /devices/revoke
- revokes a device
- invalidates its sessions

## Spaces and membership

### GET /spaces/{space_id}
- returns public metadata, rules reference, authority set reference, membership policy

### POST /spaces
- creates a space
- produces signed governance object(s)

### POST /spaces/{space_id}/join
- join intent or join request depending on policy

### POST /spaces/{space_id}/leave
- leave intent

### GET /spaces/{space_id}/membership
- returns effective membership and roles for current identity

## Object publication and retrieval

### POST /objects
Publishes one or more signed objects.
Server verifies:
- session
- signature
- membership/roles
- moderation constraints
Returns acceptance/rejection per object_id.

### GET /objects/{object_id}
Returns an object by id if allowed by policy.

### GET /spaces/{space_id}/feed?cursor=...
Returns an append-only event feed for the space:
- object_id list (and optional inline objects)
- next cursor

### GET /channels/{channel_id}/feed?cursor=...
Returns channel message feed.

## Attachments (blocks)
### POST /blocks
Upload a block.
Returns block_id.

### GET /blocks/{block_id}
Download a block if allowed.

Block storage may be mirrored to peer-to-peer replication.

## Realtime relay

### WS /realtime
WebSocket relay for:
- new object announcements
- channel message delivery
- notifications

Relay must require authentication for non-public spaces and enforce moderation constraints where feasible.

## Notifications

### GET /notifications?cursor=...
Fetch notification events (if stored server-side).

Notifications may be derived client-side from feeds; storage is optional.

## Error and rejection format
Write endpoints must return structured reasons, including:
- invalid_signature
- not_a_member
- role_missing
- banned
- content_quarantined
- rate_limited
- invalid_schema
