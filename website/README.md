# Website

This folder contains the public-facing website for Humanity
(https://shaostoul.github.io/Humanity).

The website is a presentation layer only.

---

## Authority Rules

The website is **not authoritative**.

Canonical truth lives in:
- `accord/` — civilizational principles and human-facing law
- `design/` — technical constraints, systems, schemas
- `data/` — structured, machine- and human-readable truth

The website must reflect those sources, not redefine them.

---

## Content Policy

- Website pages should either:
  - render canonical documents directly, or
  - lightly wrap them with navigation and context

- Do **not**:
  - rewrite Accord articles by hand
  - duplicate Design documents with edits
  - introduce new rules, definitions, or interpretations

If the website content diverges from the canonical documents,
the canonical documents are correct and the site must be updated.

---

## Intended Build Pattern

Preferred (future-proof):
- Website pages pull content from markdown files in `accord/` and `design/`
- Navigation and layout live here
- Content lives upstream

Acceptable (temporary):
- Website pages include short summaries that link to canonical files
- Summaries must not introduce new meaning

---

## Generated Mirror

During the Pages build, canonical docs are copied into:

website/_canon/

Files under `_canon/` are generated build material:
- must not be edited by hand
- must match canonical sources exactly
- may be deleted and regenerated at any time


---

## Why This Exists

This structure prevents:
- silent philosophical drift
- accidental divergence between “what we say” and “what we mean”
- maintenance fatigue over time

Humanity must remain coherent across decades, not just versions.

---

## Contribution Note

If you want to change meaning, change the source document.
If you want to change presentation, change this folder.
