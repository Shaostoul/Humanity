# Social vs Game State Boundary

## Purpose
Define which systems are part of the social network and which are part of the game shard authority.

## Social network state (network-governed)
- spaces, channels, threads, posts
- direct messages
- membership, roles, moderation logs
- notifications
- attachments used for social content

## Game shard state (shard-governed)
- shared economy, trading, currencies
- PvP ranking and competitive ladders
- shared world resources and permissions
- guild progression that affects others

## Shared identity
The same user identity key is used to:
- sign social objects
- authenticate to shard services
Shard services must still enforce shard authority rules.

## Offline-first relationship
- Social objects can be queued offline and delivered later.
- Shard-authoritative game events cannot be advanced offline for shard impact.
- Local-only game play uses local timeline and merge rules.

## Non-negotiable constraint
No offline-generated state may directly alter shard economy or shared competitive outcomes without server acceptance.
