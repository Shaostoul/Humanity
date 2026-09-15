# 01-VISION

## Mission

End poverty and unite humanity, by giving every person (and every AI) free tools to
meet their own needs: water, energy, food, shelter, knowledge, and a voice. Not a
startup, infrastructure for civilization. See `../ROADMAP.md` for the full mission
statement and the strategic themes it breaks into.

## Product shape

We are intentionally building **both**:

1. A real-world learning + coordination platform
2. A game world that uses the same underlying skill systems

The game is not a throwaway mini-layer. The platform is not just a launcher for a
game. They share the same simulation code in `src/systems/` (farming, crafting,
skills, etc.). The two realities are separated by NAVIGATION, not by a mode switch:
the app chrome is always real, and the game is entered through Play. The "Real/Sim
toggle" that earlier drafts of this document described was deleted from the product
in v0.197.0; see docs/design/two-realities.md for the model that replaced it.

## Design doctrine

- **Shared foundations, one crate.** `src/` is a single Rust crate; feature flags
  (`native`, `relay`, `wasm`) decide what's compiled in, not crate boundaries. Domain
  logic (math, physics, materials, progression) lives in focused modules under
  `src/systems/`, `src/relay/storage/`, etc., organized by responsibility, kept
  loosely coupled by convention and code review, not by Cargo's dependency graph.
  Splitting into real sub-crates is a possible FUTURE step once the boundaries have
  proven stable in practice, not a current requirement, don't force it prematurely.
- **Data-first development.** Anything that can exist more than once (items, quests,
  recipes, planets, ...) is a data file, not a hardcoded array, see
  `../design/infinite-of-x.md`. New domains should be addable by dropping in data,
  not by writing new match arms.
- **Explainability first.** New contributors (human or AI) must understand intent from
  docs without prior context.
- **Text-editor friendly.** No required IDE assumptions for understanding
  architecture.
- **No backwards-compatibility debt before launch.** Nobody uses this yet (operator
  directive, 2026-06-30). Change formats and APIs outright rather than carrying
  compatibility shims, revisit this once real users exist.

## What "done right" looks like

- A newcomer can understand the architecture in under 30 minutes (start at
  `00-START-HERE.md`, then `../../CLAUDE.md`).
- A new feature domain can be added mostly as data (see `../design/infinite-of-x.md`)
  plus a focused module, not a cross-cutting rewrite.
- The docs describe what's actually built. When code and docs disagree, the code and
  `../../CLAUDE.md` win, see `06-SOURCE-OF-TRUTH-MAP.md`.
