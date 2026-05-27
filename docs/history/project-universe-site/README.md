# Project Universe — shaostoul.com site archive

Static snapshot of the **Project Universe** website (https://shaostoul.com),
the 2017–2025 WordPress-era predecessor of HumanityOS. Captured **2026-05-26**
via the WordPress REST API (`scripts/archive-wordpress.js`) before retiring the
old site. Nothing redacted — the site was always public.

## Contents
- `pages/` — 22 site pages (`.html` rendered content + `.txt` plain text)
- `posts/` — 77 blog posts / docs (`.html` + `.txt`)
- `index.json` — manifest of all 286 items (99 text + 187 media): title, slug, date, source URL

## Media (NOT in git)
The 187 images/files (~264 MB — 2020-era concept art, renders, diagrams,
DALL·E pieces) are **not** committed here. They're staged locally (gitignored
`_pu-archive/media/`) and destined for the relay's file catalog (see
`docs/design/file-catalog.md`). `index.json` records every media item's
original `src` URL and archived `file` name, so they can be re-fetched or
matched any time.

## What's inside (categories)
- **Game design** — power chain (nuclear reactors → steam → breaker rooms →
  substations → rooms), atmosphere (atmo boxes, tanks-within-tanks), mining
  drones, inventory, human equipment slots, the mall, solar resource
  distribution, maps, double doors. (Directly relevant to HumanityOS systems.)
- **Mission / philosophy** — The Birth of Project Universe, Abolish Patents
  Help Humanity, Unconditional Love, Millennial Quest, To Be or Not To Be,
  Aspirations #1–5.
- **AI-collaboration series** — ChatGPT & Claude.AI intros to Humanity / AI /
  Aliens / Community (2024). Early AI-involvement history.
- **Technical** — database hierarchy/architecture (2025), PET-plastic →
  3D-filament recycling, hacked-accounts post-mortem.
- **Personal** — bio/history, 11 daily routines, 1975 Chevy Nova repair logs.
- **Org** — Sponsor-A-Can volunteer waiver + cart-patrol SOP; Fam pages
  (donors / volunteers / organizations).
- **Legal** — 20 boilerplate policies (ToS, GDPR, CCPA, COPPA, privacy, etc.).

## Re-running the archive
```
node scripts/archive-wordpress.js https://shaostoul.com C:/Humanity/_pu-archive [texts|media|all]
```
Dependency-free (Node stdlib). Pulls every page, post, and media item from the
WordPress REST API; media filenames are `<wp-id>-<slug>.<ext>` (lossless — the
id prevents collisions, the extension is preserved).
