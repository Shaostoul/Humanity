# Knowledge Tab Architecture (Markdown Explorer)

## Purpose
Provide a public-facing, easy-to-navigate markdown explorer for Humanity workspace docs without exposing sensitive/private files.

---

## User Experience

- Left: collapsible file tree
- Center: rendered markdown content
- Right: doc metadata, outline, related links

Goal: quick comprehension for users with limited patience/experience.

---

## Content Sources (Allowlist)

Initial allowlist:
- `design/*.md`
- `knowledge/*.md`
- selected root docs (e.g., `README.md` if present)

Denylist examples:
- private memory files
- credentials, keys, local secrets
- hidden/system config not intended for public reading

---

## Core Features

1. Tree navigation with folders/subfolders
2. Full-text search across allowlisted markdown
3. Rendered markdown with heading anchors
4. Right-panel outline for quick section jumps
5. Related-doc suggestions by path/topic similarity
6. Copy link to section (path + heading anchor)

---

## Data Model (minimal)

```json
{
  "path": "design/ship_zoning_transit.md",
  "title": "Ship Zoning & Transit Architecture",
  "updatedAt": "2026-03-06T00:00:00Z",
  "tags": ["systems", "transit", "design"],
  "headings": ["Goal", "Layered Spatial Model", "Transit Architecture"]
}
```

---

## Security Rules

- File access must be path-restricted to allowlist roots.
- No arbitrary path reads from UI input.
- Render markdown safely (sanitize HTML/script content).
- Expose metadata only for allowlisted docs.

---

## Naming Decision

Top-nav label recommendation: **Knowledge**

Alternative labels:
- Codex
- Library
- Docs

Rationale: "Knowledge" is broad, approachable, and future-proof.

---

## MVP Build Steps

1. Add Knowledge tab shell (3-panel layout)
2. Implement allowlisted markdown tree
3. Implement markdown renderer + heading outline
4. Add search and section links
5. Add related-doc sidebar entries
