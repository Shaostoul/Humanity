# Server Federation

> **Mixed status.** Trust tiers, `GET /api/federation/servers`, and signed
> profile gossip (Dilithium3, see CLAUDE.md "Cryptography" table) are shipped
> (`src/relay/handlers/federation.rs`, `src/relay/api.rs::list_federation_servers`).
> The signed `.well-known/humanity-servers.json` root registry and the
> verification-code operator flow described below are forward design, not yet
> implemented — servers are added to the local registry today without that flow.

## Purpose

Define how multiple independently-operated servers form the Humanity Network. Servers are meeting places, not gatekeepers. Identity is portable, the same Dilithium3 (`did:hum:`) key works on any server. Users choose which servers to join; servers choose which servers to trust.

**Key principle: No home servers.** Identity lives in the cryptographic key, not on any server. A copy of each person's profile, signed by their own app, replicates to the servers linked with the ones they use. See `docs/design/identity.md` for the full identity architecture.

---

## Core Principles

1. **Anyone can host a server.** No permission required.
2. **Servers are sovereign.** Each sets its own rules, channels, and moderation.
3. **Identity is portable.** One keypair works everywhere. No per-server accounts.
4. **Trust is earned, not assumed.** A tiered trust model signals reliability to users.
5. **The Accord is voluntary.** Adopting it publicly signals alignment with shared values.
6. **Federation is opt-in.** Servers can operate in isolation or join the network.
7. **Profiles replicate, not centralize.** A signed copy of each profile is kept by the linked servers; the latest signed copy wins.

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

## Server Registry (forward design, not yet implemented)

A signed JSON document maintained by the root authority, served at a well-known URL:

```
https://united-humanity.us/.well-known/humanity-servers.json
```

No code serves this file today; `GET /api/federation/servers` returns the
relay's own `federated_servers` DB table instead (populated ad hoc, not from a
signed root registry). The schema below is the design target if/when a signed
root registry is built.

### Schema

```json
{
  "version": 1,
  "updated": "2026-02-10T00:00:00Z",
  "root_key": "<dilithium3-public-key-hex-of-root-authority>",
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
  "signature": "<dilithium3-signature-hex-of-servers-array>"
}
```

The registry, once built, should be:
- Signed by the root authority's Dilithium3 key (tamper-proof).
- Fetched by clients on startup (cached locally, refreshed periodically).
- Small enough to embed in the client as a fallback.

### Unverified Server Discovery

Unverified servers are NOT in the root registry. They are discovered via:
- Direct URL sharing (someone gives you a link).
- A community-maintained public list (future: decentralized).
- Client-side "Add Server" with a URL input.

Unverified servers are stored locally in the client's IndexedDB.

---

## Signed Profile Replication (working since 2026-10-12, BUG-191)

Profiles are NOT owned by any server. The copy of a person's profile that other servers keep is
signed by the person's own app, never by a server, so any server can check it and none has to be
trusted for it. It goes to the servers linked with the one the person uses.

> **Before 2026-10-12 none of this worked (BUG-191).** No app signed a profile: the relay stored
> and gossiped each save with an empty signature, every linked server refused it (an unsigned
> profile is refused since 2026-08-13), and the time on it was the origin server's clock. The
> unsigned copies a server stored then are deleted when it first starts on the fixed build. Plan
> and as built: `docs/design/bug-191-profile-copies-plan.md`.

### What the copy holds

The copy is the profile as anyone may see it (`src/relay/core/profile_copy.rs` `public_copy`; the
website's twin is `web/shared/profile-copy.js`):

- name, bio, avatar, banner and links, always;
- pronouns, location and website, left blank when the person keeps them for friends;
- every field blank (a "withdrawal") when the person is unlisted.

A field kept for friends never leaves the server, signed or not. "Accept files from" is never in
it: that is the server's own setting.

### Protocol

1. **The person saves** (`profile_update`). The server stores the profile. It stores no copy for
   other servers yet.
2. **The server asks.** Its echo, the person's own complete copy sent to each of their signed-in
   devices, carries `signed_at` (the time of the copy it holds for that device's key, 0 for none)
   and `sign_now: true` when it wants a new copy (`src/relay/handlers/profile.rs` `ask_for_copy`,
   deciding by `profile_copy::sign_wanted`). On connect, and on the person's own request for their
   profile, it asks only when the copy out there shows more than the person now allows (they went
   unlisted, or keep a field for friends that the copy still shows), so a device that connects
   never pushes an older profile over a newer save made elsewhere.
3. **The person's app signs.** A device that was asked builds the public copy of exactly what the
   server stored, dates it after the newest seen (`next_copy_time`: after its own clock, after
   `signed_at`, and after the last copy it signed for that server), signs the words with its
   Dilithium3 key and sends a `profile_sign` frame (`at` and `signature`) on the same socket.
   The desktop app does this in `src/gui/pages/own_profile.rs` (`take_own_copy`), the website in
   `web/chat/chat-profile.js` (`signProfileCopy`). Nothing is signed without `sign_now`, and the
   desktop app signs nothing while its identity is locked.
4. **The server checks and keeps** (`handle_profile_sign`). It keeps the copy only when it asked
   that key in the last minute, the copy it rebuilds from what it stored keeps the limits, the time
   is at most a day ahead of its clock (otherwise the person is told to set their clock right), the
   copy is newer than the one held, and the signature checks over its own rebuild. Then it stores
   it in `signed_profiles`, replacing the whole row.
5. **The server passes it on** (`src/relay/handlers/federation.rs` `gossip_profile`): one
   `profile_gossip` to every linked server.
6. **A receiving server checks** (`should_accept_profile_gossip`, in the ProfileGossip arm of
   `handle_peer_message`): a signature, the same limits, and the owner's signature over the words.
   It keeps the copy only when it is newer than the one held, replacing the whole row. A withdrawal
   for a key it never held is kept nowhere. A copy it newly keeps for one of ITS OWN accounts goes
   on to its other links, never back to the one it came from.
7. **Catch-up when a link comes up** (`send_own_copies`): each side sends the other its copies for
   its own accounts, withdrawals included, newest first, at most 1,000, once per link. So a server
   that was down when someone went unlisted, or kept a field for friends, learns it when the link
   returns.
8. **Lookup**: `GET /api/profile/{key}` on any server answers the copy it holds. It answers not
   found for a withdrawal, and for a key whose account HERE is unlisted here. A field the person
   keeps for friends here is left out, by their choice as it is now. No app reads a copy from
   another server yet: each server's chat shows only its own profiles.

**One copy per key, network-wide; the latest wins.** A person with accounts on two linked servers
has one copy out there, the last one signed. A field they keep for friends on either server is
blanked at their next connect there.

**Going unlisted, and erasing your account.** Going unlisted makes the copy a withdrawal. Each of
the person's devices whose key has a copy out there is asked to sign one (at once when it is
signed in, otherwise at its next connect), and the withdrawal replaces the copy on every linked
server that has it. A person's own erase (`account_delete`) carries a withdrawal their app signed,
which the server passes on before it erases (`withdraw_on_erase`). An admin's erase of someone
else cannot withdraw it, because only the person's own key can sign one.

### Message Format

```json
{
  "type": "profile_gossip",
  "public_key": "<Dilithium3 public key, 3,904 lowercase hex characters>",
  "name": "Shaostoul",
  "bio": "Building HumanityOS",
  "avatar_url": "https://...",
  "banner_url": "",
  "socials": "{\"github\":\"Shaostoul\"}",
  "pronouns": "",
  "location": "",
  "website": "",
  "timestamp": 1760000000001,
  "signature": "<Dilithium3 over the words below, 6,618 lowercase hex characters>"
}
```

`socials` is the JSON text exactly as the server stored it, never re-serialized: its key order is
part of the words. The words signed, as UTF-8 (`profile_copy::copy_text`, pinned by the test
`the_copy_text_is_pinned`), with `timestamp` in decimal milliseconds:

```
profile_v1\n{public_key}\n{name}\n{bio}\n{avatar_url}\n{banner_url}\n{socials}\n{pronouns}\n{location}\n{website}\n{timestamp}
```

No field but the bio may hold a line break, so the words split one way only. A withdrawal has an
empty name, every other field empty and `socials` set to `{}`. The shared fixture
`scripts/tests/fixtures/profile-copy-v1.json` holds the desktop app and the website to these bytes.

### Gossip Rules

- A server sends only copies of its own accounts (keys registered to a name there): the ones their
  apps signed, the ones it passes on, and its catch-up. A copy it only received, for someone not
  registered there, is that person's own server's to send.
- Copies go to every linked server. A link exists only with a server the operator granted tier 2
  or above: the outbound dial and the inbound hello both require it.
- Profile copies have a per-peer allowance since 2026-10-22, like every kind of federated frame
  (`docs/design/federation-activation.md`, Phase 2): 500 at once, then 100 a second; over it the
  peer is slowed, then paused, and nothing is dropped. A copy is sent only when its owner's app
  signed a newer one, a receiver passes on only what it newly kept, and the catch-up goes once per
  link (again only when a linked server asks for it, after a pause or a gap).
- Every copy a server sends is numbered in its outbox, so a linked server that was down or paused
  catches up on exactly what it missed (`docs/design/federation-activation.md`, Phase 3).
- Receiving servers keep a copy only if:
  - it carries a signature that verifies as its owner's Dilithium3 signature over the words;
  - it keeps the limits a save holds (`profile_copy::within_limits`): the key is 3,904 lowercase
    hex characters; the name is empty (a withdrawal) or 1 to 24 of letters, digits, `_` and `-`;
    the bio is at most 280 characters with no `<` or `>`; links are a JSON object of at most 1,024
    bytes; avatar and banner at most 512 bytes and website at most 256, each empty or `https://`;
    pronouns at most 64 characters and location at most 128; no line break outside the bio;
  - it is newer than the copy held, and at most a day ahead of the receiver's clock.
- There is no separate size limit: the field limits are the limit. The key and the signature alone
  are 10,522 hex characters, so a copy is about 11 KB to 16 KB as sent.

### Key cards (2026-10-22)

A person's key card (their Kyber768 key signed by their own identity, docs/design/key-cards.md)
travels the same way as a profile copy: `{"type":"key_card_gossip","card":{...}}`, sent when a
server keeps a card newly (from a sign-in or a link), never back to where it came from; a server
passes on only the cards of its own accounts, and sends its accounts' newest 1,000 cards when a
link comes up, beside the profile copies (once per link). The receiver checks the card itself
(signed by the very key it names) and keeps it by the keep rule (`key_card::keep_rule`): the same
key changes nothing, a different key only when newer and at most a day ahead. No server is trusted
for a card: every app checks it again before it seals anything with it.

### Federated Message Persistence

Incoming `FederatedChat` messages are persisted in the local `messages` table, tagged with `origin_server` to distinguish from local messages, with the author's key in the stored frame (`from_key`, since 2026-10-22). Messages are never lost on server restart.

### The numbered log, and moderation across shared rooms (2026-10-22)

Everything a server sends its links (chat lines, edits, deletes, account erases, bans and unbans,
profile copies, signed objects) is numbered in its outbox and applied by the receiver once and in
order; a receiver that missed numbers (down, restarting, or pausing a peer that sent too fast)
asks for everything after its place and gets exactly that. The server a message was first posted
on decides its edits and deletes everywhere; a ban is honoured in the rooms the servers share. The
protocol, the decision and its reasons are in `docs/design/federation-activation.md` (Phases 2 and
3); the code is `src/relay/handlers/federation_log.rs` and `federation_flow.rs`.

The words a server signs for a chat line it sends (`fed_chat`) name the author's key since
2026-10-22, so both servers of a link must run a version from after that date for chat to cross.

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
- Server switching is fast, close old WS, open new WS, re-identify.

### Identity Across Servers

- Same Dilithium3 keypair used everywhere; same DID (`did:hum:<base58(BLAKE3(pubkey)[..16])>`) resolves on every federated server.
- The copy of your profile that other servers keep is signed by your own app when the server asks
  (after a save, or on connect when the copy out there shows more than you now allow), and goes to
  the servers linked with that one (see Signed Profile Replication above).
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

### Phase 1: Server Switching (MVP: complete)
- Client knows about multiple servers.
- Connects to one at a time.
- Server list with trust tiers.

### Phase 2: Profile Gossip + Message Persistence (current)
- Signed profile replication across federated servers (working since 2026-10-12; before that no
  app signed a profile and every copy was refused, BUG-191).
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
- Not required, opt-in for users who want it.

---

## Security Considerations

- **Malicious servers**: An unverified server could log messages, serve malicious JS, or impersonate users. Trust tiers exist to signal this risk.
- **Key theft**: If a server operator modifies the client JS to exfiltrate private keys, users on that server are compromised. Verified servers reduce this risk. Long-term: native apps eliminate it.
- **Registry tampering**: The server registry is signed. Clients verify the signature before trusting it.
- **Sybil servers**: An attacker could spin up many unverified servers. Trust tiers and the verification process mitigate this.
- **Profile spam**: A server sends only its own accounts' copies, each signed by its owner's app, and a receiver passes on only a copy it newly kept for one of its own accounts, never back to where it came from, so nothing loops. Since 2026-10-22 a peer sending faster than its per-peer allowance is slowed, then paused, and what it sent meanwhile is asked for again afterwards (`docs/design/federation-activation.md`, Phase 2).

---

## Relationship to Other Specs

- **Identity** (`design/identity.md`): Identity is the key, not the server. Profiles replicate to linked servers.
- **Social Graph** (`social_graph.md`): Follows/friends are client-side and work across servers. DMs route P2P regardless of server.
- **Voice/Video** (`voice_video_streaming.md`): WebRTC signaling goes through the current server. Cross-server calls require Phase 3.
- **File Sharing** (`file_sharing.md`): P2P file transfer works across servers since it's direct between clients.
- **Realtime Protocol** (`realtime_relay_protocol.md`): Each server runs the same protocol independently.
- **Wallet** (`design/wallet.md`): Identity key (Dilithium3) is decoupled from the optional Solana wallet (Ed25519, derived from the same BIP39 seed via path `hum/solana/v1`).

## When a peer's key changes (2026-10-11)

A server's key is its identity, pinned by its peers at first contact. If it changes (as every
server's did on 2026-09-06, when server identity moved from Ed25519 to Dilithium3), its peers
refuse its hello until they pin the new key, and the link drops within a second of each try. The
log then says, once, that the peer may not trust this server's current key. To fix it, on EACH
server, re-check the other's key: Server Settings > Federation > Re-check key (desktop), the Re-check
key button in the Admin page's federation list (web), `/server-recheck <server_id>` in chat, or
the admin API's `recheck` action. It re-reads the key from the peer's own /api/server-info over
HTTPS and pins it; the next connection uses it. Both servers must run a version from after
2026-09-06. A peer added by key alone (no web address) is removed and added again with its new key.
(BUG-192: the united-humanity.us and public.guide link was down for five weeks this way.)
