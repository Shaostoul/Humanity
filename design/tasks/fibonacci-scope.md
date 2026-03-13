---
title: Fibonacci Civilization Scale — Task Scope Model
category: design
status: active
updated: 2026-03-13
---

# Fibonacci Scope Model

Tasks and goals are organized by the scale of civilization they affect,
mapped to the Fibonacci sequence. This mirrors how complexity and coordination
requirements grow naturally — just as the Fibonacci spiral appears throughout nature.

## Why Fibonacci?

The sequence 1, 1, 2, 3, 5, 8, 13, 21, 34, 55 maps remarkably well to
human coordination scales. A task affecting one person requires almost no
coordination. A task affecting a city requires 8× more coordination infrastructure.
A civilization-scale task requires orders of magnitude more.

The spiral aesthetic reflects this: zooming out from the center reveals ever-larger
structures, each containing and building upon the previous.

## Scale Definitions

| Fib | Key      | Label   | People Scale | Description |
|-----|----------|---------|--------------|-------------|
| 1   | `self`   | Self    | 1            | Personal health, body, daily needs, sleep |
| 1   | `mind`   | Mind    | 1            | Learning, mental health, skills, creativity |
| 2   | `hearth` | Hearth  | 2–3          | Home, intimate relationships, household |
| 3   | `circle` | Circle  | 3–8          | Friend group, small team, pod |
| 5   | `village`| Village | 8–21         | Neighborhood, working group, squad |
| 8   | `city`   | City    | 21–89        | Project-level, organization, city council |
| 13  | `region` | Region  | 89–377       | Large org, national, bioregion |
| 21  | `world`  | World   | 377+         | Civilization-wide, planetary coordination |
| 34  | `solar`  | Solar   | Multi-planet | Solar system — space stations, Luna, Mars |
| 55  | `cosmos` | Cosmos  | Interstellar | Interstellar civilization |

## Implementation

### Scope as Task Label
Scope is stored in the task's `labels` JSON array:
```json
["scope:city", "backend", "relay"]
```

If no scope label is present, tasks default to `city` (project level).

### API Usage
```bash
# Get all project-level tasks
curl /api/tasks | jq '[.tasks[] | select(.labels | contains("scope:city"))]'

# Create a civilization-wide goal
curl -X POST /api/tasks \
  -H "Authorization: Bearer $API_SECRET" \
  -d '{"title":"...","labels":"[\"scope:world\"]"}'
```

### Frontend Filter
The tasks page (`tasks.html`) provides a scope selector. Selecting a scope
filters the kanban board to show only tasks at that scale.

## Visual Design

**Color gradient**: warm (personal) → cool (cosmic)

| Scope  | Color     | Feeling |
|--------|-----------|---------|
| Self   | `#ff6b6b` | Warm red — urgent, personal |
| Mind   | `#ff9f43` | Orange — energetic |
| Hearth | `#ffd32a` | Yellow — warmth of home |
| Circle | `#0be881` | Bright green — growth |
| Village| `#05c46b` | Deep green — community |
| City   | `#0fbcf9` | Sky blue — open, collaborative |
| Region | `#7f8fa6` | Steel — structured |
| World  | `#a29bfe` | Violet — visionary |
| Solar  | `#74b9ff` | Space blue — expansive |
| Cosmos | `#dfe6e9` | Near-white — infinite |

**Spiral icon**: Golden/nautilus spiral used as the Tasks page logo and
as a decorative background element on the scope selector.

## Future: True Spiral UI

Phase 3+ vision: the scope selector becomes an interactive golden spiral.
- Center point = Self
- Each concentric quarter-turn = next Fibonacci scale
- Tasks appear as cards along their ring
- Pinch/zoom gesture moves between scales
- A task can be "promoted" outward (affects more people) or "focused" inward

This would be a canvas or SVG-based component, potentially the most visually
distinctive part of the HumanityOS interface.

## Philosophical Grounding

The Fibonacci scope model enforces a design principle: **every feature should
know what scale it operates at**. A chat message is `circle` or `city` scale.
A governance vote might be `world` scale. A personal journal entry is `self`.

This prevents scope creep in both directions — personal tools don't accidentally
govern civilizations, and civilization tools don't intrude on personal space.
