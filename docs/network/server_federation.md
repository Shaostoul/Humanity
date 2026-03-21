# Server Federation

## Purpose

Define how multiple independently-operated servers form the Humanity Network. Servers are meeting places, not gatekeepers. Identity is portable — the same Ed25519 key works on any server. Users choose which servers to join; servers choose which servers to trust.

**Key principle: No home servers.** Identity lives in the cryptographic key, not on any server. Signed profiles replicate across every server a user touches. See `docs/design/identity.md` for the full identity architecture.

---

## Core Principles

1. **Anyone can host a server.** No permission required.
2. **Servers are sovereign.** Each sets its own rules, channels, and moderation.
3. **Identity is portable.** One keypair works everywhere. No per-server accounts.
4. **Trust is earned, not assumed.** A tiered trust model signals reliability to users.
5. **The Accord is voluntary.** Adopting it publicly signals alignment with shared values.
6. **Federation is opt-in.** Servers can operate in isolation or join the network.
7. **Profiles replicate, not centralize.** Signed profiles cached on every server touched; latest timestamp wins.

---

## Trust Tiers

Servers in the network are classified by two independent dimensions:

- **Verified**: The operator has been personally verified by the root authority (currently: Shaostoul).
- **Accord**: The operator has publicly adopted the Humanity Accord on their main website and social accounts.

This produces four trust levels:

| Tier | Label | Verified | Accord | Display |
|------|-------|----------|--------|---------|
| 3 | Verified + Accord | ✅ | ✅ | Green shield |
| 2 | Verified | ✅ | ❌ | Yellow shield |
| 1 | Unverified + Accord | ❌ | ✅ | Blue circle |
| 0 | Unverified | ❌ | ❌ | Grey circle |

**Display order**: Tier 3 → Tier 2 → Tier 1 → Tier 0, then alphabetical within tiers.

### Trust Tier Semantics

- **Tier 3** (Verified + Accord): Highest trust. Operator identity confirmed. Committed to the Accord's principles of dignity, transparency, and cooperation. Recommended for new users.
- **Tier 2** (Verified): Operator identity confirmed but hasn't publicly committed to the Accord. Trustworthy infrastructure, unknown alignment.
- **Tier 1** (Unverified + Accord): Operator claims Accord alignment but identity isn't confirmed. Good intent, unverified execution.
- **Tier 0** (Unverified): Unknown. Use at your own discretion. May be excellent, may be hostile.

---

## Verification Flow

### For Server Operators

1. Operator deploys a Humanity relay server.
2. Operator contacts the root authority (currently via X/Twitter DM to @Shaostoul).
3. Root authority issues a one-time verification code.
4. Operator enters the code into their server configuration.
5. Server sends a signed verification proof to the root registry.
6. Root authority confirms and the server is marked as Verified (Tier 2).
7. If the operator publicly displays Accord adoption on their main website and social accounts, they request Tier 3 and the root authority reviews and upgrades.

### Verification Code Format

```
humanity-verify:<random-32-hex>:<issued-timestamp>:<server-domain>
```

- One-time use, expires in 7 days.
- Bound to the requesting domain.
- Stored in the root registry after redemption.

---

## Server Registry

A signed JSON document maintained by the root authority, served at a well-known URL:

```
https://united-humanity.us/.well-known/humanity-servers.json
```

### Schema

```json
{
  "version": 1,
  "updated": "2026-02-10T00:00:00Z",
  "root_key": "<ed25519-public-key-of-root-authority>",
  "servers": [
    {
      "domain": "chat.united-humanity.us",
      "name": "Humanity HQ",
      "description": "The original. Home of the project.",
      "wss": "wss://chat.united-humanity.us/ws",
      "trust_tier": 3,
      "verified_at": "2026-02-10T00:00:00Z",
      "accord_verified_at": "2026-02-10T00:00:00Z",
      "operator": "Shaostoul",
      "region": "US-West",
      "tags": ["official", "development"]
    }
  ],
  "signature": "<ed25519-signature-of-servers-array>"
}
```

The registry is:
- Signed by the root authority's Ed25519 key (tamper-proof).
- Fetched by clients on startup (cached locally, refreshed periodically).
- Small enough to embed in the client as a fallback.

### Unverified Server Discovery

Unverified servers are NOT in the root registry. They are discovered via:
- Direct URL sharing (someone gives you a link).
- A community-maintained public list (future: decentralized).
- Client-side "Add Server" with a URL input.

Unverified servers are stored locally in the client's IndexedDB.

---

## Signed Profile Replication

Profiles are NOT owned by any server. They are signed objects that replicate to every server the user connects to.

### Protocol

1. **On connect**: Client sends `ProfileUpdate` with signed profile data
2. **Server validates**: Verify Ed25519 signature over canonical payload
3. **Server stores**: Cache in `signed_profiles` table with timestamp
4. **Server gossips**: Forward to all federated peers via `ProfileGossip` message
5. **Receiving server validates**: Verify signature, store if newer than existing
6. **Lookup**: Any server can serve any profile it has cached

### Message Format

```json
{
  "type": "ProfileUpdate",
  "public_key": "abc123...",
  "name": "Shaostoul",
  "bio": "Building HumanityOS",
  "avatar_url": "https://...",
  "socials": {},
  "timestamp": 1711036800000,
  "signature": "<ed25519-sig-over-canonical-payload>"
}
```

### Gossip Rules

- Only gossip profiles from users who have been seen on this server (anti-spam)
- Only gossip to Tier 2+ federated servers
- Rate limit: max 100 profile gossips per minute per server
- Receiving servers accept profile only if:
  - Signature is valid
  - Timestamp is newer than currently cached version
  - Payload size is within limits (< 10KB)

### Federated Message Persistence

Incoming `FederatedChat` messages are persisted in the local `messages` table, tagged with `origin_server` to distinguish from local messages. Messages are never lost on server restart.

---

## Client Behavior

### Server List UI

The client displays servers in the left sidebar (above channels):

```
SERVERS
🟢 Humanity HQ          ← Tier 3 (current)
🟡 Steam Community       ← Tier 2
🔵 Open Source Collective ← Tier 1
⚪ Bob's Garage           ← Tier 0
[+ Add Server]
```

Clicking a server switches the WebSocket connection and loads that server's channels, users, and pins.

### Connection Model

- Client connects to ONE server at a time (MVP).
- Future: simultaneous connections to multiple servers (like Discord).
- Server switching is fast — close old WS, open new WS, re-identify.

### Identity Across Servers

- Same Ed25519 keypair used everywhere.
- Signed profile sent on each connect — server caches and gossips it.
- Name uniqueness: per-server reservation + optional on-chain global names (see `docs/design/identity.md`).
- Block list is client-side and applies across all servers.

---

## Server Operator Guide

### Minimum Requirements

1. A domain name with TLS (Let's Encrypt is free).
2. The `humanity-relay` binary running behind a reverse proxy (nginx recommended).
3. Sufficient bandwidth and storage for your community size.

### Configuration

```toml
[server]
name = "My Server"
description = "A community for builders"
domain = "chat.example.com"
region = "EU-West"

[federation]
# Leave empty for unverified operation.
verification_code = ""
# Set to true and provide proof URL for Accord tier.
accord_adopted = false
accord_url = ""
```

### Recommendations

- **Small communities** (< 100 users): 1 CPU, 1GB RAM, 10GB disk.
- **Medium communities** (100-1000): 2 CPU, 4GB RAM, 50GB disk.
- **Large communities** (1000+): 4+ CPU, 8GB+ RAM, 100GB+ disk, consider CDN for uploads.

---

## Federation Protocol Phases

### Phase 1: Server Switching (MVP — complete)
- Client knows about multiple servers.
- Connects to one at a time.
- Server list with trust tiers.

### Phase 2: Profile Gossip + Message Persistence (current)
- Signed profile replication across federated servers.
- Federated messages persisted in storage.
- Profile lookup from any server that has cached it.

### Phase 3: Cross-Server Messaging
- Relay messages between federated servers.
- Server-to-server WebSocket connections.
- Message routing by destination server.

### Phase 4: Cross-Server Channels
- Bridged channels that span multiple servers.
- Shared moderation for bridged channels.

### Phase 5: Optional On-Chain Names
- Solana name registry for globally unique names.
- Any server verifies ownership by querying the chain.
- Not required — opt-in for users who want it.

---

## Security Considerations

- **Malicious servers**: An unverified server could log messages, serve malicious JS, or impersonate users. Trust tiers exist to signal this risk.
- **Key theft**: If a server operator modifies the client JS to exfiltrate private keys, users on that server are compromised. Verified servers reduce this risk. Long-term: native apps eliminate it.
- **Registry tampering**: The server registry is signed. Clients verify the signature before trusting it.
- **Sybil servers**: An attacker could spin up many unverified servers. Trust tiers and the verification process mitigate this.
- **Profile spam**: Gossip rules prevent amplification — only profiles from seen users are forwarded, rate-limited.

---

## Relationship to Other Specs

- **Identity** (`design/identity.md`): Identity is the key, not the server. Profiles replicate everywhere.
- **Social Graph** (`social_graph.md`): Follows/friends are client-side and work across servers. DMs route P2P regardless of server.
- **Voice/Video** (`voice_video_streaming.md`): WebRTC signaling goes through the current server. Cross-server calls require Phase 3.
- **File Sharing** (`file_sharing.md`): P2P file transfer works across servers since it's direct between clients.
- **Realtime Protocol** (`realtime_relay_protocol.md`): Each server runs the same protocol independently.
- **Wallet** (`design/wallet.md`): Same Ed25519 key is both identity and Solana wallet.
