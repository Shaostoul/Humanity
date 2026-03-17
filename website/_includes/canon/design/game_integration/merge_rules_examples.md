# Merge Rules Examples

## Purpose
Provide examples of deterministic merge rules for Bucket B (mergeable) state.

These are examples and must be expanded per system.

## Example: Lore discovery
- Local event: discovered_lore_entry(lore_id)
- Merge rule:
  - accept if lore_id exists in shard catalog
  - accept once per identity per shard
  - server assigns acceptance time/order
- Effect:
  - unlocks lore entry in shard profile

## Example: Cosmetic unlock
- Local event: unlocked_cosmetic(cosmetic_id, proof)
- Merge rule:
  - accept only if proof verifies (quest completion proof, achievement proof)
  - reject if cosmetic is shard-economy gated
- Effect:
  - cosmetic becomes available in shard character customization

## Example: Non-competitive achievement
- Local event: achievement_completed(achievement_id)
- Merge rule:
  - accept if achievement is listed as mergeable
  - accept once
- Effect:
  - award badge, not currency

## Example: Currency changes (rejected)
- Local event: currency_earned(amount)
- Merge rule:
  - always reject for shard
- Effect:
  - remains local-only timeline
