# Identity Architecture

## Core Principle

**Identity IS the Ed25519 keypair.** Not a username, not a server, not an account. The cryptographic key proves who you are. Every message, profile update, and transaction is signed by your key. No server grants identity — the math does.

## Identity Components

### Public Key = Universal Address

Your Ed25519 public key serves as:
- **Chat identity** — messages signed and attributed to this key
- **Solana wallet** — same key, base58-encoded, receives/sends crypto
- **Profile anchor** — all profile data is signed by this key
- **Cross-server ID** — same key recognized on any server

### Seed Phrase = Master Backup

BIP39 24-word mnemonic backs up everything:
- Identity keypair
- Wallet (same keys)
- Encrypted vault data
- Works across devices and platforms

## Signed Profile Replication

Profiles are NOT owned by a server. They are **signed objects** that replicate across every server the user touches.

### How It Works

1. User updates their profile (name, bio, avatar, socials)
2. Client creates a `SignedProfile` object:
   ```
   {
     public_key: "abc123...",
     name: "Shaostoul",
     bio: "Building HumanityOS",
     avatar_url: "https://...",
     socials: {...},
     timestamp: 1711036800000,
     signature: sign(canonical_payload, private_key)
   }
   ```
3. Client sends the signed profile to the current server
4. Server verifies signature, stores it, and gossips it to federated peers
5. Any server that receives a valid signed profile caches it
6. When someone looks up a key, any server that has seen the profile can serve it
7. **Latest timestamp wins** — if multiple versions exist, the newest valid one is canonical

### Properties

- **No home server** — your profile lives everywhere you've been
- **No single point of failure** — any server going down doesn't affect your identity
- **Tamper-proof** — only the private key holder can create valid updates
- **Offline-capable** — your local app always has your profile
- **P2P-compatible** — exchange profiles directly via QR/friend code

### Lookup Flow

When a client needs a profile for key X:
1. Check local cache
2. Ask the current server
3. If not found, ask known federated servers (server does this)
4. P2P: request directly from the key holder if online

## Name Uniqueness

### The Problem

Without a central authority, two users on different servers could both claim "Alice."

### Solutions (Layered)

**Layer 1: Display names are just hints (default)**
- Names are self-declared in signed profiles
- Displayed with a key fingerprint badge for disambiguation
- Clients show `Alice (7xK9)` vs `Alice (3mPq)` when ambiguous
- Works for most casual use. Signal/Session use this model.

**Layer 2: Server-scoped reservation**
- Each server enforces unique names within its own namespace
- `Alice@united-humanity.us` vs `Alice@bobs-server.com`
- Familiar to email/Matrix users
- Works without blockchain

**Layer 3: On-chain name registration (optional)**
- User registers `alice.hos` on Solana via a name registry program
- Immutable proof: this public key owns this name globally
- Costs a small SOL fee (one-time)
- Any server can verify ownership by querying the chain
- **Not required** — purely opt-in for users who want guaranteed unique names

### Recommendation

Ship Layer 1 + 2 first. Layer 3 is valuable but not blocking. Users who want global unique names can register them on-chain later.

## Server Relationship

Servers are **meeting places, not identity providers.**

- Users connect to servers for channels, tasks, and community
- Servers cache signed profiles from connected users
- Servers gossip profiles to federated peers
- A user can connect to any server without "creating an account"
- Disconnecting from a server doesn't delete your identity
- Server operators cannot impersonate users (can't sign as their key)

## Key Rotation

When a user needs to change their keypair (compromise, upgrade):

1. Sign a rotation certificate with BOTH old and new keys:
   ```
   sig_by_old = sign(new_key + "\n" + timestamp, old_private_key)
   sig_by_new = sign(old_key + "\n" + timestamp, new_private_key)
   ```
2. Broadcast the rotation to all known servers
3. Servers update the key mapping and gossip the rotation
4. Historical messages retain old key attribution but link to new identity

## P2P Identity Exchange

Works without any server:

1. User A shows QR code containing their signed profile
2. User B scans it, verifies signature, stores locally
3. Both users can now communicate via WebRTC (P2P)
4. If either connects to a server later, the friendship persists

## Related Files

- `web/chat/crypto.js` — Ed25519 key generation, signing, BIP39 seed phrase
- `web/shared/wallet.js` — Solana address derivation from same keys
- `src/relay/relay.rs` — Profile handling, key rotation
- `src/relay/handlers/federation.rs` — Server-to-server communication
- `docs/network/server_federation.md` — Federation protocol
- `docs/design/wallet.md` — Wallet integration (same keys)
