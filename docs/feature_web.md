# Feature Web Design (Project Universe)

## Goal
Build an interactive, teaching-first feature graph that maps Project Universe end-to-end:
- vision, business, school, game systems, economy, roadmap, lore, operations.
- dependencies between features/subfeatures.
- a guided way for newcomers to learn how pieces connect.

The Feature Web complements (not replaces) Kanban:
- **Kanban** = execution state.
- **Feature Web** = system understanding + roadmap topology.

## UX Principles
- Intuitive first: users should understand a node in <10s.
- Progressive depth: short summary -> teach summary -> details.
- Relationship clarity: every node should answer "what enables this?" and "what does this unlock?"
- Learn by navigation: guided paths and next-best nodes.

## V2 Scope (implemented in this pass)
1. Rich node schema beyond title/type/status.
2. Domain grouping (Vision, Business, School, Game, Economy, Tech, Lore, Roadmap, Community).
3. Inspector panel fields for summary/teach/details.
4. Teach mode with next-node suggestions.
5. Seed pack from PU doc themes.
6. Lightweight graph interactions: add/edit/link/filter/layout/export/import.

## Data Model
```json
{
  "nodes": [
    {
      "id": "n_x",
      "title": "Fleet Market",
      "type": "feature",
      "domain": "economy",
      "status": "active",
      "summary": "Public supply/demand layer for fleet-scale needs.",
      "teach": "Think of this as the whole fleet's logistics dashboard.",
      "details": "Used for repairs, quests, and large infrastructure efforts.",
      "owner": "economy",
      "priority": "high",
      "taskId": 123,
      "x": 120,
      "y": 240
    }
  ],
  "edges": [
    { "id": "e_x", "from": "n_a", "to": "n_b", "type": "depends_on" }
  ]
}
```

## Edge Types
- `depends_on`
- `blocks`
- `relates_to`
- `teaches`
- `enables`

## Teach Mode
Teach mode highlights:
- selected node teach summary,
- prerequisites (incoming edges),
- next recommended nodes (outgoing enables/teaches/depends links).

## Seed Pack (PU)
Initial graph seed includes:
- Vision + Mission
- SSPC + benefits/funding
- School stages + skill pipeline
- Core game loops
- Economy loops (market/resources/reputation)
- Act-based roadmap
- Fleet/lore anchors

## Operational Notes
- Persisted client-side for now (localStorage), with import/export JSON.
- Add server persistence in V3 (new DB tables for graph nodes/edges per project space).
- Keep board task linking in place (`taskId`) for traceability.

## V3 (next)
- Multi-user shared graph persistence via relay API.
- Presence cursors & collaborative edits.
- Permission model (viewer/contributor/mod/admin).
- Snapshot/version history and rollback.
- Graph analytics (critical path, bottleneck nodes, orphan detection).
