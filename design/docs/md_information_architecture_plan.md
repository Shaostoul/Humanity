# Markdown Information Architecture Plan

## Objective
Organize markdown files into logical folders so:
- GUI navigation (Knowledge tab) is intuitive,
- on-disk structure is pleasant and maintainable,
- root remains clean with only true parent/entry docs.

---

## Keep in Root (Parent Docs Only)

Recommended root set:
- `README.md`
- `AGENTS.md`
- `SOUL.md`
- `USER.md`
- `OPERATING_CONTRACT.md`
- `CONTRIBUTING.md`
- `PUBLIC_DOMAIN.md`
- `SELF-HOSTING.md`
- `SECURITY_AUDIT.md`
- `INTRODUCTION.md`

Move other topical markdown into domain folders.

---

## Canonical Folder Domains

- `design/` — product/system architecture and specs
- `knowledge/` — operational and reference knowledge
- `docs/accord/` — accord/legal-ethical docs (or keep existing `accord/`)
- `ops/` — deployment, runbooks, checklists
- `memory/` — session and long-term memory (private/internal)
- `website/` — web-published markdown content
- `tools/` — tool/integration docs

---

## Suggested Normalization Steps

1. Consolidate duplicates between `design/*` and `website/design/*` via source-of-truth + generated copies.
2. Keep `memory/*` excluded from public Knowledge allowlists.
3. Add `docs/index.md` and per-folder `README.md` index files.
4. Add tags/metadata header convention for better search/filter.

---

## Metadata Convention (Optional)

At top of each markdown file:

```md
---
title: Ship Zoning & Transit
category: design
tags: [systems, transit, architecture]
status: draft
updated: 2026-03-06
---
```

---

## Migration Safety

- Move files in small batches.
- Add compatibility redirects/references in old paths where needed.
- Run link checker after each batch.
- Update Knowledge allowlist after each move.

---

## "Did we miss any MD files?" Current Answer

There are many markdown files already spanning:
- root docs,
- `design/` (very large),
- `knowledge/`,
- `accord/`,
- `website/`,
- `tools/`, `desktop/`, `mvps/`, `tests/replays/`, `logs/`, `memory/`.

Nothing appears "missing" from disk; the issue is mostly discoverability and structure consistency, not absence.

---

## Immediate Next Actions

1. Create `docs/index.md` + folder indexes.
2. Define public Knowledge allowlist from this structure.
3. Run first migration batch (non-sensitive design/knowledge docs).
4. Update in-app Knowledge tree to reflect new hierarchy.
