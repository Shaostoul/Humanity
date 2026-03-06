# First/Third-Person Traversal Plan (Home + Ship)

## Goal
Enable immersive keyboard/mouse traversal through:
- private home spaces,
- public ship spaces,
- transit infrastructure (elevators, ramps, monorails),
while preserving compatibility with existing UI-driven systems.

---

## Experience Targets

1. **Seamless perspective switching**
   - first-person for precision/immersion
   - third-person for situational awareness/navigation

2. **Home-to-ship continuity**
   - player can walk from private modules to public zones without menu teleport dependency

3. **Noncombat-first foundation**
   - movement, interaction, transit, social and utility loops first
   - combat systems layered later

4. **Desktop-first controls with VR-ready architecture**
   - keyboard/mouse baseline
   - camera/controller abstraction for future VR integration

---

## Core Systems (MVP)

## 1) Player Controller
- WASD movement
- sprint / crouch / interact
- jump optional (or context-limited)
- grounded movement + slope handling

## 2) Camera System
- mode: first-person / third-person
- configurable sensitivity/FOV
- collision-safe third-person follow camera
- shoulder switch (later)

## 3) Interaction System
- raycast/volume prompts for interactables
- unified action key (default E)
- contextual actions:
  - open door
  - use terminal
  - enter elevator
  - board transit

## 4) Traversal Graph
- nav-linked zones:
  - private home modules
  - public park zones
  - civic hubs
  - industrial/service areas
- access control by utility/system state

## 5) Transit Mechanics
- elevators (vertical primary)
- ramps/stairs (vertical fallback)
- monorails (horizontal inter-hub)
- service lanes (restricted)

## 6) UI Integration Layer
- diegetic prompts + minimal HUD
- open full UI panels when interacting with terminals/kiosks
- preserve current app tabs as terminal interfaces during transition

---

## Technical Architecture

## State Layers
- `player_state`: pose, speed, mode, location
- `camera_state`: mode, fov, offsets, sensitivity
- `zone_state`: current zone, permissions, utility health impacts
- `interaction_state`: target, action options, cooldowns

## Scene Partitioning
- chunked ship sectors with streaming boundaries
- LOD by distance and zone type
- deterministic anchor points for critical modules

## Data Bindings
- module IDs should map to planning graph nodes (Feature Web / Starseed Atlas)
- utility status can gate doors/transit and module usability

---

## Control Scheme (Desktop)

- Move: WASD
- Look: Mouse
- Interact: E
- Sprint: Shift
- Crouch: Ctrl
- Perspective toggle: V
- Quick map/atlas: M
- Terminal/menu: Tab
- Emote/social: G (optional)

All keys rebindable.

---

## Phased Rollout

## Phase 1 — Greybox Traversal
- simple ship/home blockout
- first/third movement
- interactable doors + terminals

## Phase 2 — Transit Core
- elevator + ramp fallback logic
- monorail route prototype
- zone transitions and access states

## Phase 3 — Living Spaces
- home modules functional navigation
- public park corridors and civic nodes
- social gathering spaces

## Phase 4 — Utility Coupling
- outages affect movement routes and destination access
- restoration events reopen blocked functions

## Phase 5 — VR/Advanced Camera Prep
- camera abstraction hardening
- comfort options
- interaction mode parity

---

## Performance Targets (Initial)

Desktop baseline target:
- 60 FPS at target minimum hardware profile
- predictable frame-time in social/public zones
- graceful degradation in high-density areas (LOD/crowd simplification)

---

## Risks & Mitigations

1. **Scope creep in world size**
   - Mitigation: strict sector rollout and traversal corridors first.

2. **UI/gameplay split complexity**
   - Mitigation: terminals as bridge between old tab UI and world interaction.

3. **Transit complexity too early**
   - Mitigation: elevators + ramps first, monorail second.

4. **Performance in public spaces**
   - Mitigation: early profiling + level streaming + LOD constraints.

---

## Immediate Next Actions

1. Define minimal greybox layout (home + one public park corridor + one civic hub).
2. Build perspective toggle + movement controller prototype.
3. Implement interaction prompt framework.
4. Add elevator/ramp vertical traversal prototype.
5. Wire one terminal to existing UI page as proof of architecture.
