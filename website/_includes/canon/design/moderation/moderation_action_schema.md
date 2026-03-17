# Moderation Action Schema

## Purpose
Define the canonical moderation action payloads used by:
- space governance and rule enforcement
- offline-first clients enforcing safety policy
- relay edge enforcement where feasible
- auditability and attribution

Moderation actions are immutable signed objects recorded in an append-only log.
This document defines the payload of those objects.

## Scope
Applies to space-scoped moderation actions.
Platform-wide actions (if any) must be defined separately and minimized.

## Moderation object type
Moderation actions are represented as an object with:
- object_type: "moderation_action"
- space_id: required
- author_public_key: must be an authorized moderation key for this space at the time of action
- payload: the moderation action described below
- signature: required

## Authority rules
A moderation action is valid only if:
1. The object signature is valid.
2. The author_public_key is authorized by the current effective space authority set.
3. The action payload validates against this schema.
4. The action does not violate non-negotiable platform constraints.

Authority is defined by space governance objects and/or a space policy object.
Authority changes must be represented as signed governance actions.

## Canonical payload fields
Each moderation action payload contains:

- action_id: string
  - unique identifier for this action within the space
- action_type: string
  - one of the types listed in this document
- issued_at: integer
  - informational only; not trusted for ordering
- issued_by: bytes
  - the moderator public key (must match object author_public_key)
- reason: optional string
  - short human-readable reason; may be omitted for privacy
- evidence_references: optional list of object identifiers
  - references to relevant content objects where appropriate
- scope: object
  - defines what the action applies to (identity, device, content hash, channel, or membership)
- duration_seconds: optional integer
  - for temporary actions; absent means indefinite until superseded
- replaces: optional list of action_id
  - actions explicitly superseded by this action
- metadata: optional map
  - implementation-specific fields, must not affect validity decisions

## Action types

### Identity and membership actions

#### ban_identity
Prohibits an identity key from participating in the space.
- scope:
  - target_identity_public_key: bytes
- effects:
  - deny membership and posting
  - relays should refuse forwarding from banned identities for this space where feasible

#### unban_identity
Reverses a prior ban.
- scope:
  - target_identity_public_key: bytes

#### mute_identity
Limits an identity's ability to speak in channels/threads.
- scope:
  - target_identity_public_key: bytes
  - optional channel_id: string
- effects:
  - client hides or de-prioritizes delivery and display for muted identity in scope
  - relays may throttle muted identities where feasible

#### unmute_identity
Reverses a prior mute.
- scope:
  - target_identity_public_key: bytes
  - optional channel_id: string

#### remove_member
Removes membership (without necessarily banning).
- scope:
  - target_identity_public_key: bytes
- effects:
  - removes membership state; identity may reapply if allowed by policy

#### approve_member
Approves membership in spaces that require approval.
- scope:
  - target_identity_public_key: bytes

### Content actions

#### hide_content
Instructs clients not to display specific content objects.
- scope:
  - target_object_id: string
- effects:
  - clients do not display the object
  - clients may retain locally for audit, but must treat as hidden

#### quarantine_content
Instructs clients and relays not to forward or display content by default.
- scope:
  - target_object_id: string
- effects:
  - content is treated as unsafe until further review
  - clients may require user opt-in or moderator approval to view

#### allow_content
Explicitly allows content previously hidden or quarantined.
- scope:
  - target_object_id: string
- replaces:
  - should reference the action_id(s) of prior hide/quarantine actions

### Role and authority actions

#### grant_role
Grants a role to an identity.
- scope:
  - target_identity_public_key: bytes
  - role: string

#### revoke_role
Revokes a role from an identity.
- scope:
  - target_identity_public_key: bytes
  - role: string

#### update_authority_set
Changes the set of moderation authority keys.
- scope:
  - new_authority_public_keys: list of bytes
  - optional threshold: integer
- effects:
  - defines who can issue future moderation actions
  - if threshold is present, defines minimum signatures required (see multi-signature section)

#### update_space_rules
Updates space-readable rules reference.
- scope:
  - rules_reference_object_id: string
- effects:
  - binds moderation to declared rules

### Rate limiting and friction actions

#### set_posting_limits
Adjusts rate limits or friction requirements within the space.
- scope:
  - limits: object
    - messages_per_minute: optional integer
    - posts_per_hour: optional integer
    - attachments_per_day: optional integer
    - require_proof_of_work: optional boolean
    - proof_of_work_difficulty: optional integer
    - quarantine_new_identities: optional boolean
    - quarantine_duration_seconds: optional integer

## Precedence and conflict resolution
Moderation actions are applied using these rules:

1. Actions are space-scoped. Actions in one space do not apply to another space.
2. If multiple actions apply, the most restrictive effective action wins by default:
   - ban overrides mute
   - quarantine overrides allow unless allow explicitly replaces quarantine
3. unban/unmute only take effect if they replace or supersede the corresponding restrictive action.
4. Duration:
   - if duration_seconds is present, action expires after that duration.
   - expiration does not delete the action; it changes its effective status.
5. Replacement:
   - replaces explicitly marks prior actions as superseded for the same scope.

## Multi-signature support (optional, long-term)
For high-trust spaces, certain actions may require multiple moderator signatures:
- update_authority_set
- update_space_rules
- permanent bans

If enabled by space policy:
- a moderation action is valid only if it contains a list of signatures meeting the threshold.
This requires extending the base object format to support additional signatures.

## Client enforcement requirements
Clients must:
- verify signatures and authority
- apply actions deterministically
- provide user-visible attribution of actions (who signed, what scope)
- avoid downloading or rendering quarantined content by default
- allow personal block/mute independent of space moderation

## Relay enforcement requirements
Relays should:
- refuse forwarding messages from banned identities for that space
- throttle muted identities when feasible
- drop quarantined content announcements where feasible
Relays must not be treated as the ultimate source of truth; clients must still enforce.

## Privacy constraints
- reason and evidence_references are optional to avoid forced disclosure.
- spaces may define policies about what must be recorded, but must not require invasive personal data.
