# Multi-Agent Development System

> **Last updated:** v0.116.0 (2026-04-25). Agent coordination now has a runtime
> SQLite layer (`agent_sessions` table) backing the design below. See
> `data/coordination/agent_registry.ron` for the canonical scope assignments.

## Overview

Architecture for automated, parallelized development using specialized AI agents coordinated through:

1. **Static registry** (`data/coordination/agent_registry.ron`) — declares who
   owns what, what they must NOT touch, and how to detect "no work to do"
2. **Runtime state** (`agent_sessions` SQLite table) — live claim / heartbeat /
   release of scopes so multiple AI sessions don't trample each other
3. **Documentation** (this dir + STATUS.md + FEATURES.md) — the long-term
   memory that survives across sessions

Designed to survive across chat sessions, enabling any new AI session to pick up where others left off without nuking work or missing context.

---

## Coordination protocol (the safe-startup checklist)

Every freshly spun-up AI session, before touching any code, MUST:

1. **Read `agent_registry.ron`** — find the entry matching its assigned scope.
2. **Check `agent_sessions` row** for that scope_id:
   - `GET /api/v2/agents/sessions/{scope_id}` (when the API endpoint ships) OR
   - direct SQLite query during local dev
3. **Decide:**
   - If another agent's `last_heartbeat` is < `CLAIM_TIMEOUT_SECS` (30 min)
     and `state = working` → **yield**, do not touch the scope
   - If `state = completed` and `completion_check` says "no new input" →
     **passive mode**, signal coordinator that there's nothing to do
   - Otherwise → **claim the scope** via `agent_claim_scope`, set initial
     state notes describing the planned work
4. **Heartbeat every 5–10 minutes** with progress notes via `agent_heartbeat`
5. **On exit** — call `agent_release_scope` with one of:
   - `paused` — work in progress, will resume later
   - `completed` — scope at its stop state for now (e.g., elements DB has all
     ~118 elements, no new work until a new element is discovered)
   - `blocked` — waiting on input or another agent

The coordinator (the human-talking-to-AI middleman) sees aggregated status by
listing `agent_sessions` and only dispatches work to scopes that are not
actively claimed.

---

## Core Principle: Documentation IS the Coordination Layer

Every AI agent (including new chat sessions) reads the same docs before working:

```
docs/ENGINE_REFERENCE.md   -- What exists, what's missing, how to build
docs/STATUS.md             -- Feature completion inventory
docs/FEATURES.md           -- Feature file paths
docs/BUGS.md               -- Known issues, resolved bugs
docs/SOP.md                -- Version bumping, deploy procedures
docs/design/*.md           -- 87 design documents for specific systems
CLAUDE.md                  -- Project-wide instructions
```

No agent holds exclusive knowledge. If it's not in the docs, it doesn't exist.

---

## Agent Domains

Each domain owns specific directories. Agents MUST NOT modify files outside their domain without coordinator approval.

| Domain | Owns | Key Files | Priority Gaps |
|--------|------|-----------|---------------|
| **Renderer** | `src/renderer/`, `assets/shaders/` | mod.rs, camera.rs, hologram.rs, pipeline.rs | Shadow mapping, particles, post-processing, frustum culling |
| **GUI** | `src/gui/` | mod.rs, theme.rs, widgets/, pages/ | Page polish, new pages, widget library |
| **Systems** | `src/systems/` | All game systems | Combat, economy, logistics (stubs to fill) |
| **Terrain** | `src/terrain/`, `src/ship/` | planet.rs, asteroid.rs, fibonacci.rs | Terrain streaming, room materials |
| **Physics** | `src/physics/` | mod.rs, collision.rs, fluid.rs | Fluid sim, collision events |
| **Audio** | `src/audio/` | mod.rs, spatial.rs | Spatial 3D audio wiring |
| **Network** | `src/net/` | client.rs, sync.rs, protocol.rs | State replication improvements |
| **Data** | `data/` | All CSV/TOML/RON/JSON | Content expansion (items, recipes, quests) |
| **Shaders** | `assets/shaders/` | All .wgsl files | Wire unused shaders, new materials |
| **Web** | `web/` | All HTML/JS/CSS | Chat, pages, shared components |
| **Server** | `server/` | relay.rs, api.rs, storage/ | Federation, new endpoints |
| **Core** | `src/lib.rs`, `ecs/`, `assets/` | lib.rs, components.rs, AssetManager | ECS optimization, new components |

---

## How It Works Today (Claude Code Agent tool)

One main session acts as **coordinator**. Uses the `Agent` tool to spin up focused sub-agents:

```
Coordinator (this session)
  |
  +-- Agent(renderer): "Add shadow mapping pass to renderer/mod.rs"
  +-- Agent(gui): "Polish settings page slider behavior"
  +-- Agent(systems): "Implement combat damage system"
  +-- Agent(data): "Expand items.csv with 50 new furniture items"
```

### Coordinator Responsibilities
1. Read docs, understand current state
2. Decide which tasks to parallelize
3. Spin up domain agents with clear, self-contained prompts
4. Each agent prompt includes: what to build, which files to touch, what NOT to touch
5. Merge results, resolve conflicts
6. Update docs after each batch of work
7. Commit and push

### Agent Rules
- **Read docs first** -- ENGINE_REFERENCE.md, STATUS.md, BUGS.md
- **Stay in your lane** -- Only modify files in your domain
- **Commit early** -- Never accumulate large uncommitted changes
- **Update docs** -- If you add/change a system, update ENGINE_REFERENCE.md
- **No destructive git** -- Never force-push, hard-reset, or delete branches
- **Test your work** -- `cargo check` at minimum before declaring done

---

## Slash Command Design (Future: Custom Skills)

These are conceptual slash commands for agent management. Implementation path: Claude Code custom skills in `.claude/skills/`.

### `/agents status`
Show all domain agents, their last activity, and current task.

### `/agents develop <domain> "<task>"`
Spin up a domain agent to work on a specific task.
Example: `/agents develop renderer "Add shadow mapping with 2048x2048 depth texture"`

### `/agents parallel "<task1>" "<task2>" ...`
Spin up multiple agents in parallel for independent tasks.

### `/agents audit <domain>`
Run an audit agent that reads all files in a domain and reports: LOC, working/stub/missing status, suggested improvements.

### `/agents expand <data-type>`
Spin up a content generation agent to expand data files.
Example: `/agents expand recipes` -- generates 50 new crafting recipes in recipes.csv format.

### `/agents docs sync`
Re-read the entire codebase and update ENGINE_REFERENCE.md to match current state.

---

## Session Handoff Protocol

When a chat session ends, the next session picks up by:

1. Reading `CLAUDE.md` (auto-loaded)
2. Reading `docs/ENGINE_REFERENCE.md` (complete engine state)
3. Reading `docs/STATUS.md` (what's built)
4. Checking `git log --oneline -10` (recent work)
5. Checking for any `.claude/next-session-prompt.md` (explicit handoff notes)

No context is lost because all state lives in documentation, not in chat history.

---

## Future: AI-as-Platform-Citizens

Each AI agent gets an Ed25519 identity on HumanityOS. They:
- Pick up tasks from the task board API (`/api/tasks`)
- Post progress to #dev channel via WebSocket
- Commit code and submit PRs via GitHub
- Get reviewed by coordinator or human
- The task board IS the work queue
- The chat IS the coordination bus
- The repo IS the shared state

### Identity Flow
```
AI Agent spawns
  -> Generate Ed25519 keypair
  -> Register signed profile (name: "agent-renderer", role: "ai-developer")
  -> Connect to WebSocket relay
  -> Subscribe to #dev channel
  -> Poll /api/tasks?assignee=agent-renderer
  -> Work on assigned tasks
  -> Post updates to #dev
  -> Push commits, create PRs
```

---

## Safety Rules

1. Agents MUST commit early and often (never lose work to session timeout)
2. Agents MUST NOT run destructive git operations without checking for uncommitted work
3. Agents MUST read docs/STATUS.md, docs/BUGS.md before making changes
4. Agents MUST NOT modify files outside their domain
5. Agents MUST update ENGINE_REFERENCE.md when adding new systems
6. Multiple concurrent agents on the same repo require worktree isolation OR non-overlapping file domains
7. Coordinator resolves merge conflicts, not individual agents
8. Never skip version bumps -- if Rust code changes, bump minor version

---

## Content Generation Backlog

These are "obvious work" items that dedicated agents could grind through independently:

| Task | Current | Target | Agent |
|------|---------|--------|-------|
| Dictionary/glossary | 150 terms | 2,000+ terms | Data |
| Item database | 306 items | 1,000+ items | Data |
| Recipe database | 227 recipes | 500+ recipes | Data |
| Quest content | 4 quest chains | 20+ chains | Data |
| NPC dialogue | 0 | 50+ NPCs with trees | Data |
| Planet shaders wired | 2 (sun, basic) | 10+ (all planets) | Shaders |
| Procedural materials wired | 4 types | 10+ types | Shaders |
| Surface material shaders | 10 written, 0 wired | All 10 wired | Shaders |
| Tutorial content | Basic | Comprehensive | Data |
| Policy/law templates | 0 | 50+ templates | Data |
