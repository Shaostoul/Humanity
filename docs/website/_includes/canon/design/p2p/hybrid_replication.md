# Hybrid Replication

## Purpose
Define what is replicated peer-to-peer, what remains centralized, and the safety constraints of replication.

## Eligible replicated data classes
Phase order:
1. Attachments (files, images) as content-addressed blocks.
2. Public archives (read-only snapshots of public spaces).
3. Optional: public signed objects for threads and posts.

Private space content may be replicated only as encrypted objects and only among authorized members.

## Central components retained
- account validity and device revocation
- bootstrap nodes for discovery
- relay fallback for NAT and web clients
- abuse throttling at relay edge
- optional indexing/search

## Safety constraints
- Replication does not imply endorsement.
- Clients must verify signatures and apply moderation logs before display.
- Deletion cannot be guaranteed after replication.
- Private confidentiality relies on encryption and key control, not deletion.

## Peer poisoning defenses
- Reject invalid blocks by hash mismatch.
- Reject invalid signatures for objects.
- Maintain peer scoring and disconnect abusive peers.
