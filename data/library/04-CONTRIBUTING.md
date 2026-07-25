# 04-CONTRIBUTING

## Goal

Make it easy for humans and AIs to contribute safely with minimal onboarding.

## Before coding

1. Read [`00-START-HERE.md`](./00-START-HERE.md)
2. Read [`02-ARCHITECTURE.md`](./02-ARCHITECTURE.md) and `../../CLAUDE.md`
3. Check `docs/FEATURES.md`, don't rebuild what exists
4. If your change shifts strategic scope, update `docs/ROADMAP.md` in the same
   change (there is no separate ADR/`docs/decisions/` process, decisions live in
   `data/coordination/orchestrator_state.json`'s `recent_decisions` and, for
   session-level narrative, `docs/history/<date>.md`)

## PR/change checklist

- [ ] Change is scoped to one clear concern
- [ ] Relay build checked too: `cargo check --features relay --no-default-features`
      (not just native, CI deploys with the relay feature set)
- [ ] Public APIs are documented with intent
- [ ] Tests included (or rationale documented)
- [ ] Docs updated for behavioral changes (`docs/FEATURES.md`, `docs/PRIORITIES.md`,
      `docs/ROADMAP.md` as applicable)
- [ ] No em dashes in any doc prose (project-wide style rule)
- [ ] No backwards-compatibility shims added for their own sake (nobody uses this
      yet, see CLAUDE.md's Working norm; change formats outright instead)

## Documentation style

- Prefer short sections and bullets
- Define terms before using abbreviations
- Keep examples concrete
- Assume zero prior project context

## AI collaboration contract

If you are an AI contributor on this project specifically, see `../../CLAUDE.md`'s
"Working norm" section, it is the actual operating contract and supersedes generic
caution below where they conflict:

- State assumptions clearly
- Prefer additive, reviewable commits, but don't hold back a broad rewrite just
  because it's broad, if the operator directed the work, finish it, this project's
  norm is "do the whole thing," not "ask before a big diff"
- Leave handoff notes in plain language (the journal, not a chat recap)
