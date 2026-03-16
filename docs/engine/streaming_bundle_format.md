# Streaming Bundle Format

## Goal
Support offline baseline + async online updates without GitHub dependency.

## Bundle Types
- engine-core (restart required)
- content-core (hot/warm reload)
- locale/ui packs
- optional HD graphics packs

## Manifest
Each bundle includes:
- id, version, channel
- compatibility range (min/max runtime)
- hash + signature
- dependency list
- reload tier (hot/warm/restart)

## Runtime Flow
1. Load baseline bundled content offline.
2. Fetch signed manifest in background.
3. Download changed bundles only.
4. Verify hash/signature.
5. Apply by tier.

## Storage
- Keep current + previous bundle for rollback.
- Garbage collect old bundles by retention policy.

## Needs Decision
- Preferred bundle container (zip/zstd custom pack).
- Delta patch requirement in v1 or v2.