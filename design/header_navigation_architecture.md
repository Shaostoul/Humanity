# Header Navigation Architecture (Proposed)

## Problem
Current top-level navigation is overloaded and inconsistent. Related functions are split across many tabs, causing discoverability and cognitive load issues.

## Objectives
- Reduce top-level choices.
- Group by intent, not implementation.
- Keep advanced tools accessible via secondary navigation.
- Support both Fleet and Earth twin modes cleanly.

---

## Proposed Top-Level Header

1. **Home**
2. **Twin**
3. **Systems**
4. **Create**
5. **Social**
6. **Learn**
7. **Ops**
8. **Settings**

---

## Section Definitions

## Home
- personal dashboard
- quick status cards
- recent activity

## Twin
- Fleet / Earth mode switch
- Starseed Atlas / Feature Web
- map views (2D/3D)

## Systems
- utilities (power/water/network)
- logistics/transit
- industrial/refinery status
- outage and restoration state

## Create
- workshop/fabrication
- garden planner
- structure and vehicle planning

## Social
- chat/messages
- groups/community
- mall/public hubs access

## Learn
- school stages
- guided pathways
- docs/tutorials and teach mode

## Ops
- Kanban tasks
- monitoring/debug/admin surfaces
- deployment/health summaries (role-gated)

## Settings
- profile/preferences
- privacy/data controls
- keybindings/accessibility

---

## Migration Mapping (from current tabs)

- Map / Board / related planning -> **Twin**
- Reality/Fantasy/Browse dashboards -> **Home** and **Twin/Create** split
- Streams -> **Social** (or dedicated icon in Home quick actions)
- Debug/Source/Info -> **Ops** (with role gating)
- Market -> **Social** (public) or **Systems** (if tied to economy instrumentation)

---

## UX Rules

1. Keep top-level <= 8 items.
2. Put specialized tools in subnav panels.
3. Do not expose role-restricted pages as primary nav for all users.
4. Preserve stable URL routes while refactoring labels/placement.
5. Ensure mobile has same IA with progressive disclosure.

---

## Rollout Plan

1. Add new header IA behind feature flag.
2. Add section landing views with old content embedded.
3. Migrate one legacy tab cluster at a time.
4. Remove deprecated top-level tabs after parity verification.

---

## Immediate Recommendation

Start with a soft relabel + grouping pass before hard route changes:
- Keep routes stable.
- Change header labels and cluster content cards.
- Validate with user flow tests.
