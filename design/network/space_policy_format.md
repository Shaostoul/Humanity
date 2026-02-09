# Space Policy Format

## Purpose
Define the canonical space policy document that binds:
- membership policy
- roles and capabilities
- authority set
- moderation thresholds
- anti-spam settings

Space policy is readable and verifiable.
Clients use space policy to determine effective permissions and defaults.

## Representation
Space policy is a signed immutable object:
- object_type: "space_policy"
- space_id: required
- author_public_key: must be the current owner key or authorized governance key
- payload: policy data
- signature: required

Policy updates are new policy objects referencing the prior policy object.

## Payload fields

### Policy identity
- policy_version: integer
- previous_policy_object_id: optional string
- rules_text_reference_object_id: optional string (human-readable rules)
- published_at: optional integer (informational)

### Membership
- membership_policy: string
  - open
  - request_to_join
  - invite_only
  - closed
- require_profile_fields: optional list of strings (discouraged; must be minimal)

### Authority
- owner_public_key: bytes
- moderator_public_keys: list of bytes
- administrator_public_keys: optional list of bytes
- authority_threshold: optional integer
  - if present, certain actions require multi-signature

### Roles and capabilities
- roles: map role_name -> role_definition
  role_definition:
  - capabilities: list of capability strings
  - is_default_for_members: optional boolean

Capabilities are from a fixed enumeration:
- read_content
- create_threads
- create_posts
- send_messages
- upload_attachments
- react
- report
- invite_members
- approve_members
- moderate_content
- moderate_members
- manage_roles
- manage_rules
- manage_authority_set

### Safety and anti-abuse defaults
- limits:
  - messages_per_minute: optional integer
  - posts_per_hour: optional integer
  - attachments_per_day: optional integer
  - max_attachment_bytes: optional integer
- friction:
  - quarantine_new_identities: optional boolean
  - quarantine_duration_seconds: optional integer
  - require_proof_of_work: optional boolean
  - proof_of_work_difficulty: optional integer
- visibility:
  - allow_public_read: optional boolean
  - allow_public_discovery: optional boolean

## Validation rules
- Owner and authority keys must be present.
- Roles must include at least:
  - owner, administrator, moderator, member
- Default member role must grant read_content at minimum.
- Policy must not require invasive personal data.

## Precedence
If policy conflicts with signed moderation actions:
- moderation actions apply immediately
- policy defines defaults and capability structure

## Forward compatibility
- policy_version must be present
- unknown fields must be ignored only if canonical encoding preserves them safely
