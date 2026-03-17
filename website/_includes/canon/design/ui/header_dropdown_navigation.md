# Header Navigation with Dropdowns (Finalization Draft)

## Goal
Make top navigation intuitive by:
- reducing visible top-level clutter,
- using dropdowns for related subpages,
- keeping **H** as Dashboard/North-Star overview.

---

## Top-Level Header

- H (Dashboard)
- Private
- Public
- Ops (role-gated)

Optional: keep direct Chat quick button if needed.

---

## H = Dashboard

H should open a command dashboard with:
- current priorities
- blockers
- system health
- active quests by timescale
- quick action shortcuts into subpages

H is not a random home splash; it is operational overview.

---

## Private Dropdown

Suggested submenu:
- Profile
- Quests
- Calendar
- Logbook
- Inventory
- Equipment
- Skills
- Home

These are user-specific data/workflows.

---

## Public Dropdown

Suggested submenu:
- Network (chat/streams/groups)
- Systems (infrastructure and status)
- Maps (Fleet/Earth)
- Market (kiosks/services)
- Learn (onboarding/school)
- Knowledge (docs/codex)

These are shared/public ecosystem surfaces.

---

## Ops Dropdown (Role-Gated)

- Admin tools
- Debug/health
- Deploy status
- Moderation controls

Hidden or disabled for non-eligible roles.

---

## UX Rules

1. Keep top-level count low.
2. Group by user intent, not implementation details.
3. Make dropdown labels plain language.
4. Maintain keyboard accessibility for dropdowns.
5. Preserve deep-link routes for all submenu pages.

---

## Example Header Composition

`[ Private ▼ ]   [ H Dashboard ]   [ Public ▼ ]   [ Ops ▼ ]`

Alternative if alignment preference remains:
- left = Private dropdown
- center = H Dashboard
- right = Public dropdown (+ Ops)

---

## Dashboard Quick Cards (recommended)

- Resume Last Task
- Today Quests
- Legacy Quests
- Utility Alerts
- Team Activity
- Learn Next

Each card deep-links to a dropdown destination.

---

## Migration Plan

1. Implement dropdown shell while preserving existing routes.
2. Remap current top-nav buttons into Private/Public submenu buckets.
3. Add dashboard cards pointing to remapped pages.
4. Remove redundant direct top-level buttons after parity check.
