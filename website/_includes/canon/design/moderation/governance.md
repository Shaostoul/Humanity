# Governance and Moderation

## Purpose
Define space governance, authority declaration, and moderation as verifiable state.

## Space sovereignty
- Each space declares:
  - rules
  - authority set (owner/admin/moderator keys)
  - membership policy
  - anti-abuse thresholds

## Signed moderation log
Moderation actions are append-only and signed by authorized keys.
Action types include:
- ban/unban identity keys
- mute/limit identities
- hide/quarantine content object hashes
- role grants/revocations
- membership approvals/removals
- rule and authority updates

## Client enforcement
Clients must:
- verify signatures on moderation actions
- apply actions deterministically
- refuse to display or relay hidden/quarantined content according to policy
- present attribution for moderation actions (who signed)

## Appeals and process
Spaces define their own appeal process.
Platform requirements:
- actions are attributable
- rules are readable
- authority is declared
