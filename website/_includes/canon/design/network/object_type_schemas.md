# Object Type Schemas

## Purpose
Define payload schemas for initial object types so validation is stable.

All objects follow the Object Format document.
This document defines the payload contents per object_type.

## Common fields
Unless stated otherwise, payload fields are:
- deterministic
- minimal
- suitable for canonical encoding
- safe to validate offline

## Forum

### thread_create
Payload:
- title: string
- body: bytes (plaintext or ciphertext, depending on space privacy)
- body_format: string (e.g., "markdown_v1")
- tags: optional list of strings

References:
- none required

### post_create
Payload:
- body: bytes
- body_format: string

References:
- thread_object_id: string
- reply_to_post_object_id: optional string

### post_edit
Payload:
- body: bytes
- body_format: string

References:
- edited_post_object_id: string

## Chat

### channel_create (optional)
Payload:
- name: string
- topic: optional string
- visibility: optional string (policy constrained)

### message_create
Payload:
- body: bytes
- body_format: string
- attachments: optional list of block_id

References:
- channel_id: string
- reply_to_message_object_id: optional string

### message_edit (optional)
Payload:
- body: bytes
- body_format: string

References:
- edited_message_object_id: string

## Reactions

### reaction_add
Payload:
- reaction: string

References:
- target_object_id: string

### reaction_remove
Payload:
- reaction: string

References:
- target_object_id: string

## Membership

### membership_invite
Payload:
- invite_type: string ("identity" or "code")
- target_identity_public_key: optional bytes
- invite_code_hash: optional bytes
- expires_in_seconds: optional integer

### membership_request
Payload:
- message: optional string

## Moderation
Moderation payload is defined in:
- design/moderation/01_moderation_action_schema.md

## Notes
- Payload encryption for private spaces is defined in encryption documents.
- Server-side derived objects (notifications) are optional and should be minimized.
