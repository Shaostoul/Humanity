# Hybrid Network With Mandatory Relay Fallback

## Status
Accepted

## Context
Humanity OS requires communication that:
- works offline-first
- works for desktop and game clients
- works from a web browser without installed software
- can operate in centralized and decentralized modes
- resists spam and abuse
- remains reliable across real-world networking constraints

Direct peer-to-peer connectivity is unreliable for many users due to common network configurations that block inbound connections.
Web browsers also have stricter networking limits than native applications.

A pure peer-to-peer network would fail reliability requirements without relays and bootstrap infrastructure.
A pure centralized network would fail resilience goals and would concentrate operational and trust risk.

## Decision
Adopt a hybrid network architecture with a transport ladder:

1. Direct peer-to-peer transport when available.
2. Relay-assisted peer-to-peer transport when direct connectivity fails.
3. Server relay transport as a mandatory fallback for reliability, especially for web clients.

The system must support decentralized distribution of selected data classes (such as public archives and attachments) while retaining centralized components required for:
- account validity and device revocation
- abuse throttling at the relay edge
- discovery bootstrap and connectivity fallback
- optional indexing/search services

## Consequences

### Positive
- Reliable operation for most users in real-world network conditions.
- Web client remains fully functional.
- Decentralized replication is possible without making reliability depend on it.
- Operational cost can be minimized while keeping resilience pathways.

### Negative
- Requires maintaining relay infrastructure.
- Creates a partial central dependency for realtime and web access.
- Adds architectural complexity compared to purely centralized designs.

### Non-negotiable requirements created by this decision
- A server relay endpoint must exist and be maintained.
- The protocol must tolerate intermittent connectivity and relay switching.
- Peer-to-peer participation must not be required to use the platform.

## Rejected alternatives

### Pure peer-to-peer
Rejected due to reliability failures for a meaningful portion of users and poor browser compatibility without relays.

### Pure centralized
Rejected due to weaker resilience, higher trust concentration, and reduced long-term survivability.

### Peer-to-peer only for everything including identity and moderation
Rejected due to unacceptable spam/abuse risk and inability to guarantee governance consistency without strong constraints.

## Follow-up tasks
- Define the data classes eligible for decentralized replication.
- Define relay requirements and abuse controls at the relay edge.
- Document browser transport limitations and the required relay-first mode for web.
