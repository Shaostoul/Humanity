# Camera Modes Architecture (Traversal + Planning)

## Goal
Support multiple camera styles across gameplay and planning so users can choose the best perspective for each task.

---

## Core Camera Modes

1. **First-Person**
- Precision interaction
- Immersive exploration
- Home/public space traversal

2. **Third-Person**
- Spatial awareness
- Character readability
- Social and movement comfort

3. **RTS/Sims Tactical**
- Top-down / high-angle command view
- Path designation and route planning
- Best for utilities, logistics, and multi-agent orchestration

4. **Astral Projection (Creative Flight)**
- Free-fly noclip planning mode (permission-gated)
- Build/inspect map layouts and feature dependencies
- Ideal for creative world-edit and debugging

5. **Cinematic/Orbit**
- Presentation and overview
- Stakeholder-friendly project walkthroughs

---

## Transition Rules

- Perspective switching should be immediate but state-safe.
- Tactical and Astral modes can be role/permission-gated.
- Interaction model changes by mode:
  - FPS/TPP: direct avatar interaction
  - RTS: selection + command issuance
  - Astral: free camera + placement/edit tools

---

## Feature Map Visualization Modes

To move beyond card-web planning, support visual skins over the same graph data:

1. **Constellation Map** (default strategic)
- nodes as stars
- links as arcs
- glow by status/priority

2. **World Tree**
- trunk/branches layout for dependency teaching

3. **City/Zoning Overlay**
- modules mapped to districts and transit lines

4. **Card Inspector**
- detail-edit mode (current form-based workflow)

---

## MVP Implementation Order

1. Add map view mode switch (Card <-> Constellation)
2. Add tactical camera prototype (top-down pan/zoom)
3. Add astral flight prototype in planning scenes
4. Add world-tree layout renderer
5. Add city/zoning skin renderer

---

## Design Notes

- Keep one canonical node/edge data model.
- Renderers are skins; data should not fork by camera mode.
- Ensure mobile fallback uses simplified tactical + inspector views.
