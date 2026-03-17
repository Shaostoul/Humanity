# Social Graph: Follows, Friends, and Direct Messages

## Purpose

Define how users discover, follow, befriend, and privately communicate with each other across servers — transitioning from server-mediated communication to peer-to-peer direct connections.

## Principles

- **Identity is portable.** Your Ed25519 keypair is yours. It works on any server.
- **Your social graph is yours.** Follow lists are stored client-side, not controlled by any server.
- **Mutual consent for DMs.** Both parties must follow each other (friends) before direct messaging.
- **Servers are meeting places, not gatekeepers.** You discover people on servers; your relationships transcend them.
- **Privacy by default.** Follow lists are not broadcast. Only you know who you follow unless you choose to share.

## Concepts

### Follow

A unidirectional relationship: "I want to know when this person is around."

- Stored locally on the follower's device(s)
- The followed person is NOT notified (no "X followed you" — privacy first)
- Represented as: `{ public_key, display_name, added_at, servers_seen_on[] }`
- Synced across the user's own linked devices (encrypted, via their own relay or backup)

### Friend

A bidirectional follow: both parties follow each other.

- Unlocks: direct messaging, presence sharing, P2P connections
- Discovery: when you follow someone, your client can optionally signal availability for friendship (see Handshake below)
- Neither party can force friendship — either can unfollow at any time

### Friend Handshake

When user A follows user B and both are on the same server:

1. A's client sends a `friend_offer` to B (via server relay, encrypted to B's public key)
2. B's client receives the offer and checks: "Do I also follow A?"
3. If yes → automatic friend acceptance, both clients exchange connection info
4. If no → offer is silently stored; if B later follows A, friendship completes
5. Offers expire after 30 days of no mutual follow

The handshake is **opt-in per client settings**. Users can disable auto-offers and require manual friend requests instead.

### Contacts List

The local database of follows + friends + metadata:

```
contacts {
    public_key: Ed25519 public key (primary identifier)
    display_name: string (cached, may be outdated)
    relationship: "follow" | "friend" | "blocked"
    added_at: timestamp
    last_seen_at: timestamp
    servers: [server_url, ...]  // where you've seen them
    notes: string               // personal notes, never shared
    profile_cache: {bio, socials}  // cached from last seen
}
```

Stored in IndexedDB (web client) or local SQLite (native client). Encrypted at rest.

## Direct Messages (DMs)

### Requirements

- Both users must be friends (mutual follow)
- End-to-end encrypted (XChaCha20-Poly1305)
- Messages are signed (Ed25519)
- Work P2P when both online; relay-fallback when one is offline

### Key Exchange

On becoming friends, clients perform an X25519 key exchange:

1. Each party derives an X25519 public key from their Ed25519 key (per RFC 8032 / libsodium)
2. Shared secret computed via X25519 Diffie-Hellman
3. Session keys derived via BLAKE3 KDF from the shared secret
4. Keys are rotated periodically (ratchet protocol — future spec)

### Message Routing (3-tier)

**Tier 1 — Direct P2P:**
- If both parties are online and reachable (same LAN, or public IP, or hole-punched)
- Lowest latency, no third party involved
- Connection negotiated via ICE-like signaling through the relay

**Tier 2 — Relay-forwarded:**
- If direct connection fails (NAT, firewalls)
- Messages sent through a relay server both parties trust
- Relay sees encrypted blobs only — cannot read content
- Relay stores messages for offline delivery (encrypted, time-limited)

**Tier 3 — Store-and-forward:**
- If recipient is offline
- Sender's relay (or a mutually trusted relay) stores the encrypted message
- Recipient retrieves on next connect
- Messages expire after configurable TTL (default: 30 days)

### DM Data Model

```
direct_message {
    id: BLAKE3 hash of (sender_key + recipient_key + timestamp + nonce)
    from: Ed25519 public key
    to: Ed25519 public key
    timestamp: u64 (ms since epoch)
    nonce: 24 bytes (XChaCha20-Poly1305)
    ciphertext: encrypted payload
    signature: Ed25519 signature over (ciphertext + timestamp + nonce)
}
```

The plaintext payload contains:
```
dm_payload {
    content: string
    attachments: optional [content-addressed references]
    reply_to: optional message_id
}
```

## Server Interaction

### Server-scoped features

Servers provide:
- Community channels (public/private)
- User discovery (see who's in the server)
- Friend handshake relay (forwarding encrypted offers)
- Presence aggregation (who's online — for server members only)
- Moderation within their space

Servers do NOT provide:
- Authority over identity
- Authority over who can be friends
- Access to DM content
- Control over the social graph

### Multi-server presence

A user connected to servers A, B, and C appears online on all three. Their client maintains separate WebSocket connections. The contact list tracks which servers a friend was last seen on, for reconnection hints.

### Server discovery

Users can share server URLs. Servers can optionally publish to a public directory (future spec). No central registry — servers are autonomous.

## Scaling Considerations

### Small scale (< 1,000 users per server)
- Full user list in sidebar: works fine
- Follow/friend handshakes via server relay: minimal overhead
- DMs relay-forwarded through the server

### Medium scale (1,000 – 100,000)
- Sidebar shows: friends online + current channel members
- Paginated user search for discovery
- DM relay: dedicated message queue per user pair

### Large scale (100,000+)
- Presence subscriptions: subscribe to friends only, not all users
- Sharded user directories
- Multiple relay servers per community (load balanced)
- P2P DMs reduce server load as network grows

### Billions scale
- Federated server mesh for discovery
- DHT-based friend routing (find which relay a friend is connected to)
- Client-side social graph means no single point has the full picture
- Servers are leaves, not roots — the network is the platform

## Privacy Guarantees

- **Follow list**: never leaves your device(s) unencrypted
- **Friend offers**: encrypted to recipient's key, relay cannot read
- **DMs**: end-to-end encrypted, relay sees only ciphertext + metadata
- **Metadata minimization**: relays should not log sender/recipient pairs long-term
- **Plausible deniability**: future spec may add deniable authentication

## Migration Path from Current MVP

1. **Phase 1 (now):** Server-scoped user list, admin/mod reports
2. **Phase 2:** Local contacts/follow list in IndexedDB, `/follow` and `/unfollow` commands
3. **Phase 3:** Friend handshake protocol, mutual follow detection
4. **Phase 4:** DMs via server relay (encrypted)
5. **Phase 5:** P2P direct connections (WebRTC data channels for web, raw TCP/QUIC for native)
6. **Phase 6:** Multi-server identity, cross-server friend discovery
7. **Phase 7:** Store-and-forward for offline DMs, message expiry
8. **Phase 8:** Key ratcheting, forward secrecy

## Open Questions

- Should follow counts be public? (Leans no — avoid popularity metrics)
- Should there be a "request to follow" mode for private accounts? (Possible future addition)
- Group DMs: separate spec or extension of this one? (Separate — different key management)
- Should relays charge for store-and-forward? (Possible sustainability model)
