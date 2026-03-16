# Retention and Deletion Semantics

## Purpose
Define what "delete" means in an immutable-object and replication-capable system.

## Core constraint
If data is replicated to independent nodes, the platform cannot guarantee global deletion.
Deletion therefore means:
- removal from display by policy
- removal from indexes
- removal from centralized storage under platform control where possible
- prevention of future distribution where feasible

## Types of deletion

### Local deletion
A client may delete its local cached copy of content.
This does not affect other nodes.

### Central storage deletion
The platform may delete objects or blocks from platform-controlled storage.
This does not delete content from independent peers.

### Policy deletion (hide/quarantine)
A signed moderation action can hide or quarantine content.
Clients must not display hidden content.
Relays should not forward quarantined content where feasible.

### Cryptographic deletion
For private content, key rotation can prevent future access to content encrypted under new keys.
Previously distributed keys still allow decryption of old content.

## Retention policies
Spaces may define retention:
- retain forever
- time-limited retention (e.g., 90 days)
- message history limits per channel

Retention affects:
- what the server keeps
- what indexes keep
It does not guarantee deletion from peers.

## User expectations (must be stated)
- Public contributions may be retained even after leaving.
- Private spaces protect content via encryption and membership control.
- Replication reduces guarantees of deletion.
