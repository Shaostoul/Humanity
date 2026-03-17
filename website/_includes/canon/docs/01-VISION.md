# 01-VISION

## Mission

Build a public-domain ecosystem that helps people learn, build, and collaborate through practical tools and interactive systems.

## Product shape

We are intentionally building **both**:

1. A real-world learning + coordination platform
2. A game world that uses the same underlying skill systems

The game is not a throwaway mini-layer. The platform is not just a launcher for a game.

## Design doctrine

- **Shared foundations, separate orchestration**
  - Shared domain crates handle math, physics, materials, and progression.
  - Platform and game clients orchestrate those crates differently.
- **Module-first development**
  - Domain modules should run independently for tests and iteration.
- **Explainability first**
  - New contributors (human or AI) must understand intent from docs without prior context.
- **Text-editor friendly**
  - No required IDE assumptions for understanding architecture.

## What “done right” looks like

- A newcomer can understand the architecture in under 30 minutes.
- A contributor can add a module without touching unrelated systems.
- Math/physics logic can be reused by web app features and game mechanics.
- Docs remain current with code via ADRs and module readmes.
