# Transport Security

## Purpose
Define minimum transport-layer and protocol-layer protections:
- replay resistance
- token binding and session safety
- abuse controls at relays
- privacy constraints for telemetry

## Replay resistance
- Each signed object includes an object_id derived from hash.
- Duplicate objects are harmless and must be deduplicated by object_id.
- Server event feeds are cursor-based to prevent replay amplification.

For endpoints that mutate server state outside object publication (device enrollment, revocation):
- require a per-request nonce or server challenge that is signed by the client
- reject reused nonces within a window

## Session token requirements
- short-lived access tokens
- refresh tokens stored securely and rotated
- tokens bound to device enrollment where feasible
- immediate invalidation on device revoke

## Relay abuse controls
Relays must implement:
- connection quotas per identity and per IP
- message rate limits per identity and per space
- burst limits and backpressure
- temporary bans for abusive patterns
- resource caps per connection (memory, queued messages)

Relays should enforce active moderation decisions where feasible.

## Privacy-preserving telemetry
Collect only what is required to operate and defend:
- aggregate counts
- coarse-grained metrics
- abuse signals without storing content where possible

Prohibited by default:
- long-term storage of message contents for analytics
- relationship-graph analytics for engagement optimization

## Secure defaults
- TLS required for server endpoints.
- Certificate pinning is optional; if used, must include rotation plan.
- Clients verify signatures regardless of transport security.

## Browser considerations
Web clients cannot rely on background processing and have storage constraints.
Relay-first operation is required for realtime in browsers.
