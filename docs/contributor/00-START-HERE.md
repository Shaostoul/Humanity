# 00-START-HERE

If this is your first day in this repo, start here.

## 1) What this project is

HumanityOS is a Rust workspace that compiles into one binary: the root package at `src/` and
the crates cut out of it so far under `crates/` (the crate split, `docs/design/crate-split.md`:
ten crates decided on 2026-10-30, the first two cut on 2026-11-01), plus a plain HTML/JS web
frontend (`web/`) that mirrors it. Feature flags (`native`, `relay`, `wasm`) decide what's
included. It is
both a real-world learning/coordination platform and an educational game, the two
share the same underlying survival/production/skill systems rather than being
separate products.

> **Not a multi-crate module system.** Earlier drafts of this contributor doc set
> (this file, `01-VISION.md`, `03-MODULE-MAP.md`) described an aspirational
> multi-crate architecture (`module-firearms`, `module-water-systems`, etc., each its
> own crate with dependency-isolation rules). That was never built and, per the
> operator (2026-06-30), was not the direction then. (The split into ten crates by layer,
> decided 2026-10-30, `docs/design/crate-split.md`, is a different thing: it cuts along the
> dependency layers to make builds faster, not into per-feature modules.) Feature domains
> (farming, crafting, combat, electrical, ...) live as modules INSIDE `src/systems/`
> and `src/relay/storage/`, not as separate crates. If you see a doc, comment, or AI
> agent still framing this as multi-crate, that description is stale.

Read next: `../../CLAUDE.md` for the real architecture, file map, and build commands.

## 2) Ground rules

- Keep modules small and composable inside their crate; the crate boundaries are the ten
  of `scripts/crate-layers.json`, not one per feature.
- Prefer plain Markdown docs and readable Rust over clever abstractions.
- Every folder should be understandable in a basic editor (Notepad++, terminal,
  Obsidian).
- Avoid hidden magic and undocumented side effects.
- No backwards-compatibility shims right now, nobody uses this yet (operator
  directive, 2026-06-30, see CLAUDE.md's Working norm section). Change formats
  outright instead of preserving old ones.

## 3) Fast orientation (10 minutes)

1. Read `../../CLAUDE.md`, the real source of truth: architecture, file map, version
   SOP, the canonical cryptography table, the non-negotiable design rules.
2. Read `../PRIORITIES.md`, the strict-ranked backlog, the top of TIER 0 is what's
   being worked on next.
3. Read `02-ARCHITECTURE.md` for the single-crate module layout in more depth.
4. Read `../ai/05-AI-ONBOARDING.md` if you are a coding agent.

## 4) Where to work first

- Product/intent questions: `docs/`
- Rust module boundaries and responsibilities inside `src/`: `docs/contributor/02-ARCHITECTURE.md`
- Feature/content planning: `docs/contributor/03-MODULE-MAP.md`

## 5) Safe first task

Pick one:

- Improve wording in one docs file for clarity.
- Fix a broken doc link (`node scripts/check-doc-links.js` finds them).
- Pick a small item from `docs/PRIORITIES.md` and scope it down further.
