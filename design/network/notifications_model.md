# Notifications Model

## Purpose
Define how notifications are produced, delivered, and stored.

Default principle:
- Notifications are derived from immutable objects and feeds.
- Server-side notification storage is optional and treated as a cache.

## Notification events
A notification is derived when an object matches rules such as:
- mention of identity in a thread/post/message
- reply to a user’s post/message
- direct message received
- moderation action affecting the user
- membership approval/removal/role change

## Delivery
- Realtime: relay sends notification hints (object_id references).
- Pull: client derives notifications while processing feed events.
- Optional: server stores notification cache per identity for convenience.

## Persistence
Clients store notifications locally for offline access.
Server may store a bounded cache with retention policy.

## Privacy constraints
- Avoid exposing private relationship graphs.
- Do not generate engagement-optimization notifications.
- Notification payloads must not include decrypted private content.
