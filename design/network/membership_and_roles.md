# Membership and Roles

## Purpose
Define membership state, role capabilities, and how membership is granted, revoked, and enforced across offline-first and hybrid transport modes.

## Definitions
- Space: a community boundary with its own rules and authority.
- Member: an identity with participation privileges in a space.
- Role: a named capability set granted within a space.
- Device: a client instance enrolled to an account (revocable).

## Identity scope
All membership and roles are granted to a user identity public key.
Devices do not hold membership directly. Devices are only a way to access an identity.

## Membership policies
Each space must declare a membership policy:
- open: anyone may join
- request_to_join: join requires approval
- invite_only: join requires invitation
- closed: no new members except by explicit owner action

Membership policy is readable prior to joining.

## Roles
### Required roles
- owner: ultimate authority for the space, can change authority set
- administrator: high authority, can manage members and rules
- moderator: can issue moderation actions within allowed scope
- member: standard participation
- limited: can read but has restricted posting
- muted: cannot speak in scoped areas (can still read)
- banned: cannot participate (membership removed or blocked)

Spaces may define additional roles, but required roles must exist.

## Capability model
Capabilities are not inferred from role names in code.
Roles map to explicit capabilities in space policy, such as:
- read_content
- create_threads
- create_posts
- send_messages
- upload_attachments
- react
- report
- moderate_content
- moderate_members
- manage_roles
- manage_rules
- manage_authority_set

Space policy defines:
- which roles grant which capabilities
- whether certain actions require multi-signature

## Membership state representation
Membership state is represented by signed objects:
- approve_member
- remove_member
- ban_identity / unban_identity
- grant_role / revoke_role

A client considers an identity an effective member if:
1. membership policy allows it and a valid approval/invite exists when required
2. there is no effective ban action
3. membership has not been removed (unless policy allows implicit rejoin)

## Invitations (if enabled)
An invitation is a signed object:
- object_type: "membership_invite"
- space_id
- issued_by (must have invite capability)
- target_identity_public_key or one-time invite code
- expiration_seconds (optional)

Invite codes must be non-guessable and single-use.

## Joining flow
### Open spaces
- user submits join intent
- server records membership (or client records local intent and syncs)
- membership becomes effective unless banned

### Request-to-join spaces
- user submits request
- moderators approve_member to activate
- until approved, user is not an effective member

### Invite-only spaces
- user presents invite
- membership becomes effective if invite is valid and not expired and identity is not banned

## Leaving flow
A user can leave voluntarily:
- produces remove_member (self-issued) or a leave intent recorded by server
- user loses membership privileges
- leaving must always be possible for a member

Spaces may retain public contributions per policy, but must respect hide/quarantine decisions.

## Device revocation interaction
If a device is revoked:
- that device cannot obtain valid session tokens
- identity membership remains unchanged
- user can enroll a new device and continue

## Offline-first behavior
- Clients may cache membership and role state locally.
- On reconnect, clients must synchronize and re-validate effective membership against signed logs.
- Offline posting requires effective membership cached and not expired by policy.
- Servers re-validate on acceptance; rejected items remain local-only.

## Non-negotiable requirements
- Membership and roles must be readable and auditable within a space.
- Moderation authority must be declared.
- Users must be able to leave spaces.
