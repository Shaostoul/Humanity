# RFC: Hybrid P2P + Relay Continuity for Messaging

Status: Draft

## Summary
Humanity currently uses a relay-centric model for messaging presence, history, and rendezvous. This RFC proposes a hybrid architecture so desktop app users can keep communicating when a primary relay is unavailable, while preserving compatibility with web clients.

## Goals
- Reduce single-relay dependency for app-to-app communication.
- Preserve DM/group continuity during temporary relay outages.
- Keep web clients functional without claiming impossible offline guarantees.
- Maintain clear security boundaries and predictable fallback behavior.

## Non-Goals
- Fully serverless web chat.
- Guaranteed delivery in all offline/NAT scenarios.
- Replacing relays entirely.

## Current Constraints
- Web clients depend on origin/relay availability.
- Presence and group/DM history are centralized.
- Peer connections can exist, but rendezvous/state is relay-driven.

## Proposed Architecture

### Layer 1: Primary Relay (current)
- Default for presence, discovery, history, moderation controls.

### Layer 2: Fallback Relay List
- Client stores an ordered list of trusted relays.
- On failure, client attempts failover in order.
- Relay identity pinned by public key fingerprint.

### Layer 3: Direct Peer Continuity (app only)
- App caches recent peer endpoints/session hints.
- During relay outage, app attempts direct reconnect for recent contacts/friends.
- Supports temporary DM continuity for reachable peers.

### Layer 4: Local Queue + Deferred Sync
- Outbound messages queued locally when unreachable.
- On reconnection to any trusted relay/peer path, queued messages sync with dedupe IDs.

## Presence Model (target)
- Servers tab: server/channel context members.
- Groups tab: active group members only.
- DMs tab: friends-only roster (mutual follow).

## Security
- Trusted relay fingerprints required for failover.
- Message IDs + signatures for replay protection and dedupe.
- No automatic trust expansion from unknown relays.

## Delivery Semantics
- Best-effort during outage with local queue.
- Eventual consistency after reconnection.
- UI must distinguish: sent, queued, delivered, synced.

## Rollout Plan

### Phase 1
- Relay failover list + trust pinning.
- Local outbound queue with dedupe IDs.

### Phase 2
- App peer endpoint cache + direct reconnect attempts.
- DM continuity during relay outage for reachable peers.

### Phase 3
- Group continuity enhancements over mixed paths.
- Conflict handling and merge telemetry.

## Open Questions
- Queue retention policy and max size.
- Group conflict resolution if split-brain occurs.
- UX for trust onboarding of additional relays.

## Success Criteria
- Relay outage does not block all app DM messaging for recently connected peers.
- Users can see explicit queued/delivered/synced states.
- Failover success rate and recovery time are observable.
