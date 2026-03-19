# Signed Moderation Logs And Space-Declared Authority

## Status
Accepted

## Context
A communication platform without verifiable governance collapses under:
- spam
- harassment
- coordinated abuse
- impersonation
- moderator abuse without accountability

Humanity OS also requires:
- longevity across time and forks
- optional decentralized replication
- offline-first clients that can enforce safety policy locally

If moderation is purely server-side and opaque:
- clients cannot reason about trust when operating offline
- decentralized replication becomes unsafe and inconsistent
- moderator abuse is harder to detect and resolve

## Decision
Moderation and governance are space-scoped and verifiable.

- Each space declares an authority set (owner/admin/moderator keys) and rules.
- Moderation actions are recorded in an append-only log.
- Each moderation action is signed by an authorized moderation key.
- Clients apply moderation logs deterministically:
  - bans/unbans
  - mutes/limits
  - hide/quarantine of specific content object hashes
  - role and permission changes

Server relay infrastructure may enforce moderation decisions at the edge, but the source of truth for moderation decisions is the signed log.

## Consequences

### Positive
- Moderation actions are attributable and auditable.
- Offline-first clients can enforce safety rules without constant server access.
- Decentralized replication can coexist with governance.
- Spaces can fork by changing authority keys and rules.

### Negative
- Adds implementation complexity.
- Requires careful key management for moderators.
- Once replicated, content cannot be guaranteed deletable; moderation becomes non-display and non-relay.

### Non-negotiable requirements created by this decision
- Spaces must publish authority and policy metadata.
- Moderation actions must be signed and append-only.
- Clients must implement deterministic enforcement.

## Rejected alternatives

### Opaque server-only moderation
Rejected due to offline-first requirements and lack of verifiable governance.

### Fully democratic moderation without declared authority
Rejected due to high abuse risk and unclear accountability.

### Global moderation only (platform-wide authority) for everything
Rejected because spaces require sovereignty and diverse rulesets.

## Follow-up tasks
- Define the moderation action schema.
- Define how clients handle conflicting or superseded actions.
- Define minimum audit and transparency surfaces for users.
