# Network Scope

## Purpose
Define the scope, surfaces, and non-goals of the Humanity OS communication and community network.

## System name
Humanity Network (implementation name). Part of Humanity OS.

## Required surfaces

### Community
- Spaces (communities)
- Membership and roles
- Rules and governance metadata per space
- Threads (forum topics)
- Posts (replies)
- Reactions
- Mentions
- Search (optional early; required long-term)

### Messaging
- Space channels
- Direct messages
- Presence (optional)
- Notifications

### Content
- Attachments (images, files)
- Content-addressed storage for objects eligible for replication

### Clients
- Game client
- Desktop client
- Web client

## Operating modes
- Offline-first: clients must function without connectivity.
- Hybrid transport: peer-to-peer may be used when available; relay fallback is mandatory.
- Central services: account validity, device enrollment/revocation, relay, abuse controls.

## Trust boundaries
- Client signs actions; server verifies.
- Private identity keys remain client-side.
- Moderation is space-scoped and verifiable by signed logs.

## Non-goals
- Guaranteed deletion in decentralized replication.
- Anonymous global chat without governance controls.
- Global consensus ledger or blockchain dependency.
- Perfect metadata secrecy.

## Success criteria
- A user can participate from desktop, game, and web with one identity.
- Offline drafting and reading works; sending is queued and synced.
- Spam and abuse are containable at space level with platform safeguards.
- Transport failures degrade gracefully without data loss.
