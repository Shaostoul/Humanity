# Transport Security

> There is no session-token or device-enrollment system in code today.
> Authentication is **per-request signature verification**: REST writes are
> authenticated by a Dilithium3 (or, for the two-phase WebSocket `identify`,
> a nonce-challenge Dilithium3) signature over a domain-separated preimage
> (see CLAUDE.md's "Key patterns" section and the Cryptography table's
> Inc3b/Inc5c-core entries). The "Session token requirements" section below
> is forward design if a token layer is ever added on top; it is not current
> behavior.

## Purpose
Define minimum transport-layer and protocol-layer protections:
- replay resistance
- abuse controls at relays
- privacy constraints for telemetry

## Replay resistance
- Each signed object includes an object_id derived from hash.
- Duplicate objects are harmless and must be deduplicated by object_id (the
  relay's `put_signed_object` is INSERT-OR-IGNORE).
- Authenticated REST writes require a fresh timestamp in the signed preimage
  (freshness window, e.g. `vault_sync` requires <=5 min) rather than a nonce.
- The WebSocket `identify` flow uses a server-issued one-time nonce challenge
  (`hum/identify/v1\n{nonce}\n{pubkey}`, see CLAUDE.md's Inc3b entry) so a
  captured `identify` cannot be replayed to impersonate the identity.

## Session token requirements (forward design, not implemented)
- short-lived access tokens
- refresh tokens stored securely and rotated
- tokens bound to device enrollment where feasible
- immediate invalidation on device revoke

## Relay abuse controls
Relays must implement:
- connection quotas per identity and per IP
- message rate limits per identity and per channel (Fibonacci backoff per
  public key, `src/relay/relay.rs`)
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
