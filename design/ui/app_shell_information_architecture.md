# App Shell Information Architecture (Private | H | Public)

## Goal
Define a stable, intuitive app shell that separates:
- user-specific workflows,
- always-available home anchor,
- public/shared ecosystem workflows.

This architecture should remain consistent across web/desktop/mobile variants.

---

## Top Header Model

## Left Cluster: Private
User-specific tabs and state.

- Profile
- Inventory
- Skills
- Equipment
- Home

## Center Anchor: H
Persistent north-star button.
- Returns user to core landing/dashboard.
- Always visible.

## Right Cluster: Public
Shared ecosystem tabs.

- Network
- Systems
- Maps
- Market
- Learn
- Knowledge (docs/codex)
- Ops (role-gated)

---

## Reusable 3-Panel Page Pattern

Each major tab should reuse this structure:

## Left Panel (Navigation Tree)
- folder/subfolder hierarchy
- fast section jumps
- compact and collapsible

## Center Panel (Primary Workspace)
- visual interaction area (graph/map/scene/list canvas)
- where core user task happens

## Right Panel (Data + Controls)
- selected item data
- metrics/status
- actions and edits

This keeps interaction predictable and reduces cognitive load.

---

## Tab Responsibilities (v1)

## Private Side

### Profile
- identity summary
- progression snapshots
- preferences/privacy quick access

### Inventory
- items/resources/stock
- filters/sorting/loadout staging

### Skills
- skill tree/list + levels + learning paths
- prerequisites and recommendations

### Equipment
- loadouts
- gear comparisons
- condition and maintenance data

### Home
- player-home modules
- module statuses and personal spaces

## Public Side

### Network
- chat/streams/groups/events
- communications and social layer

### Systems
- utility and infrastructure states
- feature showcase / dependency map
- outage/restoration and throughput context

### Maps
- Fleet/Earth maps
- route planning and overlays

### Market
- kiosk/partner access
- policy-routed integration modes

### Learn
- school pathways
- tutorials and guided onboarding

### Knowledge (Docs)
- markdown knowledge explorer across Humanity folders
- public-readable docs/codex/reference material

### Ops (role-gated)
- admin/debug/deploy/health surfaces

---

## Knowledge Tab (Docs/Codex) Proposal

Purpose: expose markdown content in a simple public-facing explorer.

### Layout
- Left: file/folder tree (`design/`, `knowledge/`, selected root docs)
- Center: rendered markdown viewer
- Right: metadata panel (path, last modified, tags, related docs)

### Scope controls
- Public allowlist for readable directories/files
- Private/sensitive files excluded by policy

### Features
- search across markdown files
- heading outline navigation
- copy/share link to doc sections
- related docs suggestions

### Naming options
- Knowledge
- Codex
- Library
- Docs

Recommendation: **Knowledge** (friendly + broad).

---

## UX Rules

1. Keep top-level tabs stable and low-churn.
2. Preserve the Private | H | Public split visually.
3. Keep center workspace dominant on desktop.
4. Collapse side panels on smaller screens with quick toggles.
5. Do not mix role-gated tools into general public tabs.

---

## Rollout Plan

1. Header regrouping into Private | H | Public clusters.
2. Apply 3-panel shell to Systems (already started).
3. Add Knowledge tab with markdown explorer.
4. Migrate Inventory/Profile/Equipment/Skills to same shell pattern.
5. Validate with quick usability tests and adjust labels.
