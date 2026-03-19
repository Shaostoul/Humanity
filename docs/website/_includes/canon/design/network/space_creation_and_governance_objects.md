# Space Creation and Governance Objects

## Purpose
Define the objects that create a space and evolve its governance over time.

Moderation actions assume a declared authority set.
This document defines how that authority set comes into existence and changes.

## Space creation
A space is created by a signed object:

### space_create
Payload fields:
- space_name: string
- description: optional string
- initial_policy_object_id: string (must reference a space_policy object)
- created_by_identity_public_key: bytes

Effects:
- establishes space_id (server assigns or derived deterministically by policy)
- binds the initial policy

## Governance updates
### space_policy
Defined in:
- design/network/08_space_policy_format.md

Policy changes are represented as new space_policy objects referencing the prior policy.

### authority updates
Authority updates are represented as:
- space_policy updates changing authority keys
and/or
- moderation_action update_authority_set

Rules:
- authority changes must be auditable
- clients must resolve which authority set is effective by applying governance objects in order from the latest accepted policy head

## Rules text
Human-readable rules must be referenced as an object:
- rules_text_reference_object_id
This object may be:
- plaintext in public spaces
- encrypted in private spaces

## Minimum governance requirements
- Every space must have:
  - an owner key
  - at least one moderator key (may be owner)
  - a readable rules reference
  - a membership policy
