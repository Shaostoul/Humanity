# Network Scope

## Purpose
Define the scope, surfaces, and non-goals of the Humanity OS communication and community network.

## System name
Humanity Network (implementation name). Part of Humanity OS.

## Required surfaces

### Community
- Servers (federated, each independently operated) and channels within them —
  see [server_federation.md](server_federation.md). There is no separate
  "space" governance-object layer in code; a `space_id` field exists only as
  an optional scoping tag on governance proposals (`src/relay/storage/governance.rs`).
- Membership and roles (server-scoped, `server_members` table)
- Reactions, pins, threads (message replies)
- Mentions
- Search (`GET /api/search`, shipped)

### Messaging
- Channels
- Direct messages (see [social_graph.md](social_graph.md))
- Presence
- Notifications

### Content
- Attachments (`POST /api/upload`, server-stored today; see
  [file_sharing.md](file_sharing.md) for the not-yet-built P2P alternative)
- Signed-objects substrate for P2P groups, governance, and credentials (see
  [object_format.md](object_format.md))

### Clients
- Native desktop client (Rust/egui)
- Web client

## Operating modes
- Hybrid transport: peer-to-peer is used for voice (shipped, see
  [native_voice.md](native_voice.md)) and is the target for P2P groups
  (`docs/design/p2p-groups.md`); relay fallback is mandatory.
- Central services: identity signature verification, relay, abuse controls.
  There is no account/device-enrollment system — identity is the Dilithium3
  keypair derived from the user's BIP39 seed.

## Trust boundaries
- Client signs actions; server verifies.
- Private identity keys remain client-side.
- Governance/moderation for the objects that have it today (P2P groups,
  governance votes) is enforced by signed, append-only logs — see
  `docs/design/signed_moderation_logs.md`.

## Non-goals
- Guaranteed deletion in decentralized replication.
- Anonymous global chat without moderation controls.
- Global consensus ledger or blockchain dependency (Solana is used only for
  the optional wallet feature, not network consensus).
- Perfect metadata secrecy.

## Success criteria
- A user can participate from desktop and web with one identity.
- Spam and abuse are containable per-channel and per-server with platform safeguards.
- Transport failures degrade gracefully without data loss.
