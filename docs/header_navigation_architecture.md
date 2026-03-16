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

1. **Private**
2. **Network**
3. **Market**
4. **Twin**
5. **Build**
6. **Learn**
7. **Ops**
8. **Settings**

---

## Section Definitions

## Private
- player-home systems (bedroom, battlestation, garden, workshop, garage)
- personal dashboard + local planning tools
- private/local data controls

## Network
- chat/messages/streams/groups/events
- public utility and collaboration surfaces
- fleet communications and social participation

## Market
- mall kiosks + partner/service catalog
- mode-aware partner routing (embed, API, external launch)
- affiliate attribution + compliance disclosures

## Twin
- Fleet / Earth mode switch
- Starseed Atlas / Feature Web
- map views (2D/3D)

## Build
- workshop/fabrication
- garden planner
- structure and vehicle planning

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
- Reality/Fantasy/Browse dashboards -> **Private** and **Twin/Build** split
- Streams -> **Network**
- Debug/Source/Info -> **Ops** (with role gating)
- Market -> **Market** (with utility/economy overlays exposed in **Twin/Network** where needed)

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
