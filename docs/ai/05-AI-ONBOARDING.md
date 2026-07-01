# 05-AI-ONBOARDING

This file is the minimum context a coding agent needs before touching code. It is a
condensed pointer, not a replacement for CLAUDE.md, if anything here and CLAUDE.md
disagree, CLAUDE.md wins.

## Project one-liner

HumanityOS is a single Rust crate (`src/`) that compiles into one binary, plus a plain
HTML/JS web frontend (`web/`) that mirrors it. Feature flags (`native`, `relay`, `wasm`)
decide what's included, there is no Cargo workspace and no sub-crates. Mission: end
poverty and unite humanity, for billions of eventual users, humans and AI both as
first-class citizens.

## First things to actually read, in order

1. `../../CLAUDE.md`, the real source of truth: session-start checklist, file map,
   version SOP, the canonical cryptography table, the non-negotiable design rules.
2. `../PRIORITIES.md`, the strict-ranked tactical backlog. The top of TIER 0 is what
   gets worked on next.
3. `../../data/coordination/orchestrator_state.json`, the running session journal:
   recent decisions, active scope claims, what NOT to redo.

## Non-negotiables (from CLAUDE.md)

- **GUI-first configurability**: anything an operator/admin/user can configure must be
  reachable in-app, not only via shell.
- **Rust-first canonical UI**: new UI patterns are implementable in native egui first;
  web mirrors it.
- **One theme source**: design tokens live in `data/gui/theme.ron`; don't hand-edit
  `theme.css`.
- **Infinite-of-X**: anything that can exist more than once is a data file, not a
  hardcoded array in code.
- **Dual-UI parity**: a new web widget/pattern gets a native port in the same
  increment, or a documented reason why not.

## New contributor quick path

1. Read `../contributor/00-START-HERE.md` for the contributor-doc reading order.
2. Check `docs/FEATURES.md` before proposing anything new, it may already exist.
3. Pick one scoped change; check `data/coordination/agent_registry.ron` for ownership
   rules if the area might have another agent's active claim.
4. Run `cargo check --features relay --no-default-features` in addition to the native
   build before pushing any Rust change, CI deploys with the relay feature set.
5. Update the relevant doc (PRIORITIES.md, FEATURES.md, or the journal) in the same
   change, not as a follow-up.
