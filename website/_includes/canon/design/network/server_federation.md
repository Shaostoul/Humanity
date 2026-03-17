# Server Federation

## Purpose

Define how multiple independently-operated servers form the Humanity Network. Servers are meeting places, not gatekeepers. Identity is portable — the same Ed25519 key works on any server. Users choose which servers to join; servers choose which servers to trust.

---

## Core Principles

1. **Anyone can host a server.** No permission required.
2. **Servers are sovereign.** Each sets its own rules, channels, and moderation.
3. **Identity is portable.** One keypair works everywhere. No per-server accounts.
4. **Trust is earned, not assumed.** A tiered trust model signals reliability to users.
5. **The Accord is voluntary.** Adopting it publicly signals alignment with shared values.
6. **Federation is opt-in.** Servers can operate in isolation or join the network.

---

## Trust Tiers

Servers in the network are classified by two independent dimensions:

- **Verified**: The operator has been personally verified by the root authority (currently: Shaostoul).
- **Accord**: The operator has publicly adopted the Humanity Accord on their main website and social accounts.

This produces four trust levels:

| Tier | Label | Verified | Accord | Display |
|------|-------|----------|--------|---------|
| 3 | Verified + Accord | ✅ | ✅ | 🟢 Green shield |
| 2 | Verified | ✅ | ❌ | 🟡 Yellow shield |
| 1 | Unverified + Accord | ❌ | ✅ | 🔵 Blue circle |
| 0 | Unverified | ❌ | ❌ | ⚪ Grey circle |

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

### Accord Verification

Accord adoption is verified by the root authority checking:
1. The operator's main website displays a link to the Humanity Accord with explicit acceptance language.
2. At least one major social account (X, YouTube, etc.) references the Accord.

This is a human-reviewed process, not automated.

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
- Name registration is per-server (you could be "Alice" on one server and find the name taken on another).
- Profile data (bio, socials) synced from client on each connect.
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

## Federation Protocol (Future)

### Phase 1: Server Switching (MVP — current target)
- Client knows about multiple servers.
- Connects to one at a time.
- Server list with trust tiers.

### Phase 2: Cross-Server Identity
- Name reservation across verified servers (optional, cooperative).
- Profile portability — server forwards profile to peers.

### Phase 3: Cross-Server Messaging
- Relay messages between federated servers.
- Server-to-server WebSocket connections.
- Message routing by destination server.

### Phase 4: Cross-Server Channels
- Bridged channels that span multiple servers.
- Shared moderation for bridged channels.

---

## Security Considerations

- **Malicious servers**: An unverified server could log messages, serve malicious JS, or impersonate users. Trust tiers exist to signal this risk.
- **Key theft**: If a server operator modifies the client JS to exfiltrate private keys, users on that server are compromised. Verified servers reduce this risk. Long-term: native apps eliminate it.
- **Registry tampering**: The server registry is signed. Clients verify the signature before trusting it.
- **Sybil servers**: An attacker could spin up many unverified servers. Trust tiers and the verification process mitigate this.

---

## Migration Path

1. Write server registry JSON with chat.united-humanity.us as the sole Tier 3 server.
2. Add "Servers" section to client UI with server list, trust badges, and "Add Server" button.
3. Add server switching (close/open WebSocket).
4. Add verification code flow to relay binary.
5. Document the process for new server operators.
6. First external server: whoever asks first.

---

## Relationship to Other Specs

- **Social Graph** (`social_graph.md`): Follows/friends are client-side and work across servers. DMs route P2P regardless of server.
- **Voice/Video** (`voice_video_streaming.md`): WebRTC signaling goes through the current server. Cross-server calls require Phase 3.
- **File Sharing** (`file_sharing.md`): P2P file transfer works across servers since it's direct between clients.
- **Realtime Protocol** (`realtime_relay_protocol.md`): Each server runs the same protocol independently.
