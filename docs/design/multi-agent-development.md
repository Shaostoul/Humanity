# Multi-Agent Development System

## Overview

Architecture for automated, parallelized development using specialized AI agents coordinated through HumanityOS itself.

## Architecture

```
User (Shaostoul)
  |
  v
Conductor Agent (orchestrator, single session)
  |
  +-- Chat Agent       (owns: web/chat/, native chat, relay messaging)
  +-- Studio Agent     (owns: streaming, WebRTC, scenes)
  +-- Garden Agent     (owns: web/activities/gardening, native farming)
  +-- Engine Agent     (owns: native/src/ renderer, ECS, physics, terrain)
  +-- Governance Agent (owns: voting, proposals, nonprofit tools)
  +-- Content Agent    (owns: data/ files, dictionaries, items, recipes)
  +-- ... one per major feature area
```

## Coordination Layers

1. **Git** -- universal sync. Agent commits, others see on pull.
2. **Memory files** -- shared context in ~/.claude/projects/.../memory/
3. **CLAUDE.md** -- loaded into every session automatically.
4. **HumanityOS chat #dev channel** -- agents post status updates as real messages visible to users and each other.
5. **Task board** -- each agent picks up assigned tasks, marks progress, posts results.

## Current Limitation

Claude Code sessions are isolated. No inter-session messaging. The conductor must relay between agents manually OR use the Agent tool to spin up specialized sub-agents within one session.

## Near-Term Approach (works today)

One main session acts as conductor. Uses the Agent tool to spin up focused sub-agents in worktrees. Each gets a scoped prompt like "you own web/chat/. Fix X." They run in parallel, commit to branches, conductor merges.

## Future Approach (build toward)

Each AI agent has an Ed25519 identity on HumanityOS. They:
- Pick up tasks from the task board API
- Post progress to #dev channel
- Commit code and submit PRs
- Get reviewed by conductor agent or human
- The task board IS the work queue
- The chat IS the coordination bus
- The repo IS the shared state

## Safety Rules

- Agents MUST commit early and often (never lose work to worktree cleanup)
- Agents MUST NOT run destructive git operations without checking for uncommitted work
- Agents MUST read docs/STATUS.md, docs/BUGS.md before making changes
- Multiple concurrent sessions on the same repo require worktree isolation
- Conductor resolves merge conflicts, not individual agents

## Content Generation Needs

These are "obvious work" items that dedicated agents could grind through:
- Full dictionary/glossary expansion (currently 150+ terms, need thousands)
- Policy/law templates for nonprofit governance
- Item database expansion (shirts, tools, vehicles, weapons, furniture, etc.)
- Recipe expansion (crafting, cooking, construction)
- Planet/star data integration (119,627 stars from stars.csv)
- Quest content generation
- NPC dialogue trees
- Tutorial content for all features
