# Anti-Spam and Abuse Controls

## Purpose
Provide durable defenses against spam, sybil attacks, and harassment across centralized and hybrid modes.

## Layered friction model
Spaces select thresholds; platform enforces minimum baseline.

Controls:
- membership gates (invite, approval, verified)
- quarantine-by-default for unknown identities in open spaces
- rate limits by identity, device, and connection
- optional proof-of-work stamps per post/message
- reputation gates (time, endorsements, completed onboarding)
- reporting and moderator queue
- block/mute at user level

## Relay edge enforcement
Relays must enforce:
- connection limits
- per-identity throughput limits
- burst controls
- temporary bans for abusive patterns
- refusal to forward content violating active moderation decisions where feasible

## Non-punitive defaults
- Prefer quarantine and limiting over mass bans.
- Provide local user controls independent of moderator action.

## Limitations
- Verified accounts reduce anonymity but do not guarantee safety.
- Replicated content cannot be guaranteed deletable; focus on non-distribution and non-display.
